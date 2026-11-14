//! Reading aloud with Kokoro-82M (Apache-2.0), on this computer.
//!
//! Text becomes phonemes with espeak-ng (loaded at run time, so Biblion
//! starts without it), and phonemes become 24 kHz audio with the Kokoro ONNX
//! model. The model and its voices are downloaded once, when the user asks.

use std::collections::HashMap;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;

pub const SAMPLE_RATE: u32 = 24_000;
/// The model's context, leaving room for the padding token at each end.
const MAX_PHONEMES: usize = 510;
const STYLE_DIM: usize = 256;

const MODEL_FILE: &str = "kokoro-v1.0.onnx";
const VOICES_FILE: &str = "voices-v1.0.bin";
const RELEASE: &str = "https://github.com/thewh1teagle/kokoro-onnx/releases/download/model-files-v1.0";
/// Expected sizes, to tell a finished download from a cut one.
const MODEL_BYTES: u64 = 325_532_387;
const VOICES_BYTES: u64 = 28_214_398;

/// Voices offered, with the espeak language that reads their text.
pub const VOICES: &[(&str, &str, &str)] = &[
    ("ef_dora", "Dora", "es"),
    ("em_alex", "Alex", "es"),
    ("em_santa", "Santa", "es"),
    ("af_heart", "Heart", "en-us"),
    ("af_bella", "Bella", "en-us"),
    ("am_michael", "Michael", "en-us"),
    ("bf_emma", "Emma", "en-gb"),
    ("bm_george", "George", "en-gb"),
];

#[derive(Debug, Clone, Serialize)]
pub struct VoiceInfo {
    pub id: String,
    pub name: String,
    pub lang: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct Status {
    /// The model and voices are downloaded.
    pub installed: bool,
    /// espeak-ng could be loaded.
    pub espeak: bool,
    pub download_bytes: u64,
    pub sample_rate: u32,
    pub voices: Vec<VoiceInfo>,
}

pub fn voices() -> Vec<VoiceInfo> {
    VOICES.iter().map(|(id, name, lang)| VoiceInfo { id: (*id).into(), name: (*name).into(), lang: (*lang).into() }).collect()
}

// ── espeak-ng ────────────────────────────────────────────────────────────

type InitFn = unsafe extern "C" fn(c_int, c_int, *const c_char, c_int) -> c_int;
type SetVoiceFn = unsafe extern "C" fn(*const c_char) -> c_int;
type ToPhonemesFn = unsafe extern "C" fn(*mut *const c_void, c_int, c_int) -> *const c_char;

/// espeak-ng keeps global state: one instance, used under a lock.
struct Espeak {
    _lib: libloading::Library,
    set_voice: SetVoiceFn,
    to_phonemes: ToPhonemesFn,
    voice: Option<String>,
}

// The library's functions are only called while holding the Mutex around it.
unsafe impl Send for Espeak {}

impl Espeak {
    fn load(bundled: Option<&Path>) -> Result<Self, String> {
        let mut candidates: Vec<PathBuf> = Vec::new();
        if let Some(dir) = bundled {
            for name in ["libespeak-ng.so.1", "libespeak-ng.dll", "libespeak-ng.dylib"] {
                candidates.push(dir.join(name));
            }
        }
        for name in ["libespeak-ng.so.1", "libespeak-ng.so", "libespeak-ng.dll", "espeak-ng.dll", "libespeak-ng.1.dylib", "libespeak-ng.dylib"] {
            candidates.push(PathBuf::from(name));
        }
        let lib = candidates
            .iter()
            .find_map(|p| unsafe { libloading::Library::new(p) }.ok())
            .ok_or("espeak-ng is not installed")?;
        // A bundled copy brings its data folder; a system one knows its own.
        let data = bundled.map(|d| d.join("espeak-ng-data")).filter(|d| d.is_dir());
        let data = data.map(|d| CString::new(d.to_string_lossy().as_bytes()).map_err(|e| e.to_string())).transpose()?;
        unsafe {
            let init: InitFn = *lib.get::<InitFn>(b"espeak_Initialize\0").map_err(|e| e.to_string())?;
            let set_voice: SetVoiceFn = *lib.get::<SetVoiceFn>(b"espeak_SetVoiceByName\0").map_err(|e| e.to_string())?;
            let to_phonemes: ToPhonemesFn = *lib.get::<ToPhonemesFn>(b"espeak_TextToPhonemes\0").map_err(|e| e.to_string())?;
            // 0x02: AUDIO_OUTPUT_SYNCHRONOUS (no sound device is opened).
            if init(0x02, 0, data.as_ref().map_or(std::ptr::null(), |d| d.as_ptr()), 0) <= 0 {
                return Err("espeak-ng could not start".into());
            }
            Ok(Self { _lib: lib, set_voice, to_phonemes, voice: None })
        }
    }

    /// IPA phonemes of plain text (no punctuation), with stress marks.
    fn phonemes(&mut self, text: &str, lang: &str) -> Result<String, String> {
        if self.voice.as_deref() != Some(lang) {
            let name = CString::new(lang).map_err(|e| e.to_string())?;
            if unsafe { (self.set_voice)(name.as_ptr()) } != 0 {
                return Err(format!("espeak-ng has no voice for {lang}"));
            }
            self.voice = Some(lang.to_owned());
        }
        let text = CString::new(text.replace('\0', " ")).map_err(|e| e.to_string())?;
        let mut ptr = text.as_ptr() as *const c_void;
        let mut clauses = Vec::new();
        // 1: UTF-8 input. 0x02: IPA output, "_" between phonemes (dropped below).
        let mode = ((b'_' as c_int) << 8) | 0x02;
        while !ptr.is_null() {
            let out = unsafe { (self.to_phonemes)(&mut ptr, 1, mode) };
            if out.is_null() {
                break;
            }
            let s = unsafe { CStr::from_ptr(out) }.to_string_lossy().into_owned();
            if !s.trim().is_empty() {
                clauses.push(s);
            }
        }
        Ok(tidy_phonemes(&clauses.join(" ")))
    }
}

/// What phonemizer does to espeak's output: one line, no phoneme separators,
/// single spaces between words.
fn tidy_phonemes(raw: &str) -> String {
    raw.replace('\n', " ").split(' ').map(|w| w.trim().replace('_', "")).filter(|w| !w.is_empty()).collect::<Vec<_>>().join(" ")
}

/// Punctuation kept as is (Kokoro reads pauses and intonation from it).
const MARKS: &str = ";:,.!?¡¿—…\"«»“”(){}[]";

/// Phonemes for a sentence, keeping its punctuation like phonemizer's
/// `preserve_punctuation`: the text between marks is phonemized, the marks
/// (and the spaces around them) stay.
fn phonemize(espeak: &mut Espeak, text: &str, lang: &str) -> Result<String, String> {
    let mut out = String::new();
    let mut chunk = String::new();
    let mut marks = String::new();
    let flush_chunk = |out: &mut String, chunk: &mut String, espeak: &mut Espeak| -> Result<(), String> {
        if !chunk.trim().is_empty() {
            out.push_str(&espeak.phonemes(chunk.trim(), lang)?);
        }
        chunk.clear();
        Ok(())
    };
    let mut pending_space = false;
    for c in text.chars() {
        if MARKS.contains(c) {
            flush_chunk(&mut out, &mut chunk, espeak)?;
            if pending_space && marks.is_empty() && !out.is_empty() {
                marks.push(' ');
            }
            pending_space = false;
            marks.push(c);
        } else if c.is_whitespace() {
            if !marks.is_empty() {
                marks.push(' ');
                out.push_str(&marks);
                marks.clear();
            } else {
                pending_space = true;
                chunk.push(' ');
            }
        } else {
            if !marks.is_empty() {
                out.push_str(&marks);
                marks.clear();
            }
            pending_space = false;
            chunk.push(c);
        }
    }
    flush_chunk(&mut out, &mut chunk, espeak)?;
    out.push_str(&marks);
    Ok(out.split_whitespace().collect::<Vec<_>>().join(" "))
}

// ── Kokoro ───────────────────────────────────────────────────────────────

struct Engine {
    session: ort::session::Session,
    voices: HashMap<String, Vec<f32>>,
    vocab: HashMap<char, i64>,
}

impl Engine {
    fn load(dir: &Path) -> Result<Self, String> {
        let threads = std::thread::available_parallelism().map_or(2, |n| n.get()).clamp(1, 6);
        let fail = |e: String| format!("could not load the voice model: {e}");
        let env = ort::environment::init().with_name("biblion-tts").with_telemetry(false).build().map_err(|e| fail(e.to_string()))?;
        let builder = ort::session::Session::builder(&env).map_err(|e| fail(e.to_string()))?;
        let mut builder = builder.with_intra_threads(threads).map_err(|e| fail(e.to_string()))?;
        let session = builder.commit_from_file(dir.join(MODEL_FILE)).map_err(|e| fail(e.to_string()))?;
        let voices = read_voices(&dir.join(VOICES_FILE))?;
        let vocab: HashMap<String, i64> = serde_json::from_str(include_str!("kokoro_vocab.json")).map_err(|e| e.to_string())?;
        let vocab = vocab.into_iter().filter_map(|(k, v)| Some((k.chars().next()?, v))).collect();
        Ok(Self { session, voices, vocab })
    }

    fn synth(&mut self, phonemes: &str, voice: &str, speed: f32) -> Result<Vec<f32>, String> {
        let style_table = self.voices.get(voice).ok_or_else(|| format!("unknown voice {voice}"))?;
        let ids: Vec<i64> = phonemes.chars().filter_map(|c| self.vocab.get(&c).copied()).take(MAX_PHONEMES).collect();
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        // The voice's style depends on the length of what it says.
        let row = ids.len().min(style_table.len() / STYLE_DIM - 1);
        let style = style_table[row * STYLE_DIM..(row + 1) * STYLE_DIM].to_vec();
        let mut tokens = Vec::with_capacity(ids.len() + 2);
        tokens.push(0);
        tokens.extend(ids);
        tokens.push(0);
        let n = tokens.len();
        let to_err = |e: ort::Error| e.to_string();
        let inputs = ort::inputs![
            "tokens" => ort::value::Tensor::from_array(([1usize, n], tokens)).map_err(to_err)?,
            "style" => ort::value::Tensor::from_array(([1usize, STYLE_DIM], style)).map_err(to_err)?,
            "speed" => ort::value::Tensor::from_array(([1usize], vec![speed.clamp(0.5, 2.0)])).map_err(to_err)?,
        ];
        let outputs = self.session.run(inputs).map_err(to_err)?;
        let (_, audio) = outputs["audio"].try_extract_tensor::<f32>().map_err(to_err)?;
        Ok(audio.to_vec())
    }
}

/// voices-v1.0.bin is a NumPy .npz: a zip of float32 arrays (510, 1, 256).
fn read_voices(path: &Path) -> Result<HashMap<String, Vec<f32>>, String> {
    let file = std::fs::File::open(path).map_err(|e| format!("cannot open {}: {e}", path.display()))?;
    let mut zip = zip::ZipArchive::new(file).map_err(|e| e.to_string())?;
    let mut out = HashMap::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(|e| e.to_string())?;
        let name = entry.name().map_err(|e| e.to_string())?.into_owned();
        let Some(name) = name.strip_suffix(".npy").map(str::to_owned) else { continue };
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
        out.insert(name, parse_npy_f32(&bytes)?);
    }
    Ok(out)
}

/// The data of a little-endian float32 .npy array.
fn parse_npy_f32(bytes: &[u8]) -> Result<Vec<f32>, String> {
    if bytes.len() < 10 || &bytes[..6] != b"\x93NUMPY" {
        return Err("not a NumPy array".into());
    }
    let (header_len, start) = match bytes[6] {
        1 => (u16::from_le_bytes([bytes[8], bytes[9]]) as usize, 10),
        _ => (u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]) as usize, 12),
    };
    let header = std::str::from_utf8(bytes.get(start..start + header_len).ok_or("short header")?).map_err(|e| e.to_string())?;
    if !header.contains("'<f4'") || header.contains("'fortran_order': True") {
        return Err(format!("unexpected voice array: {header}"));
    }
    let data = &bytes[start + header_len..];
    Ok(data.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect())
}

// ── The service ──────────────────────────────────────────────────────────

pub struct Tts {
    dir: PathBuf,
    bundled_espeak: Option<PathBuf>,
    espeak: Mutex<Option<Espeak>>,
    engine: Mutex<Option<Engine>>,
}

impl Tts {
    pub fn new(data_dir: &Path, bundled_espeak: Option<PathBuf>) -> Self {
        Self { dir: data_dir.join("tts"), bundled_espeak, espeak: Mutex::new(None), engine: Mutex::new(None) }
    }

    pub fn installed(&self) -> bool {
        let size = |f: &str| std::fs::metadata(self.dir.join(f)).map(|m| m.len()).unwrap_or(0);
        size(MODEL_FILE) == MODEL_BYTES && size(VOICES_FILE) == VOICES_BYTES
    }

    fn with_espeak<T>(&self, f: impl FnOnce(&mut Espeak) -> Result<T, String>) -> Result<T, String> {
        let mut guard = self.espeak.lock().unwrap();
        if guard.is_none() {
            *guard = Some(Espeak::load(self.bundled_espeak.as_deref())?);
        }
        f(guard.as_mut().unwrap())
    }

    pub fn status(&self) -> Status {
        Status {
            installed: self.installed(),
            espeak: self.with_espeak(|_| Ok(())).is_ok(),
            download_bytes: MODEL_BYTES + VOICES_BYTES,
            sample_rate: SAMPLE_RATE,
            voices: voices(),
        }
    }

    /// Audio for one sentence: mono float32 samples at 24 kHz.
    pub fn speak(&self, text: &str, voice: &str, speed: f32) -> Result<Vec<f32>, String> {
        let lang = VOICES.iter().find(|v| v.0 == voice).map(|v| v.2).ok_or_else(|| format!("unknown voice {voice}"))?;
        if !self.installed() {
            return Err("The voice is not downloaded yet".into());
        }
        let phonemes = self.with_espeak(|e| phonemize(e, text, lang))?;
        let mut engine = self.engine.lock().unwrap();
        if engine.is_none() {
            *engine = Some(Engine::load(&self.dir)?);
        }
        let engine = engine.as_mut().unwrap();
        // Long sentences are cut at spaces to fit the model's context.
        let mut audio = Vec::new();
        for part in split_phonemes(&phonemes) {
            audio.extend(engine.synth(&part, voice, speed)?);
        }
        Ok(audio)
    }

    /// Downloads the model and voices, reporting (done, total) bytes.
    pub fn install(&self, on_progress: &dyn Fn(u64, u64)) -> Result<(), String> {
        std::fs::create_dir_all(&self.dir).map_err(|e| e.to_string())?;
        let agent: ureq::Agent = ureq::Agent::config_builder().user_agent("Biblion/0.1 (local library manager)").build().into();
        let total = MODEL_BYTES + VOICES_BYTES;
        let mut done = 0;
        for (file, size) in [(VOICES_FILE, VOICES_BYTES), (MODEL_FILE, MODEL_BYTES)] {
            let dest = self.dir.join(file);
            if std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0) == size {
                done += size;
                continue;
            }
            let part = self.dir.join(format!("{file}.part"));
            let mut res = agent.get(&format!("{RELEASE}/{file}")).call().map_err(|e| format!("Could not download the voice: {e}"))?;
            let mut reader = res.body_mut().with_config().limit(size + 1).reader();
            let mut out = std::fs::File::create(&part).map_err(|e| e.to_string())?;
            let mut buf = vec![0u8; 256 * 1024];
            let mut got = 0u64;
            loop {
                let n = reader.read(&mut buf).map_err(|e| format!("The download stopped: {e}"))?;
                if n == 0 {
                    break;
                }
                out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
                got += n as u64;
                on_progress(done + got, total);
            }
            out.sync_all().map_err(|e| e.to_string())?;
            if got != size {
                let _ = std::fs::remove_file(&part);
                return Err(format!("The download of {file} was incomplete ({got} of {size} bytes)"));
            }
            std::fs::rename(&part, &dest).map_err(|e| e.to_string())?;
            done += size;
        }
        Ok(())
    }

    /// Deletes the downloaded model and voices.
    pub fn remove(&self) -> Result<(), String> {
        *self.engine.lock().unwrap() = None;
        for f in [MODEL_FILE, VOICES_FILE] {
            let p = self.dir.join(f);
            if p.exists() {
                std::fs::remove_file(&p).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
}

/// Splits phonemes at spaces into parts the model can take.
fn split_phonemes(phonemes: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    for word in phonemes.split(' ') {
        if current.chars().count() + word.chars().count() + 1 > MAX_PHONEMES && !current.is_empty() {
            parts.push(std::mem::take(&mut current));
        }
        if !current.is_empty() {
            current.push(' ');
        }
        current.push_str(word);
    }
    if !current.is_empty() {
        parts.push(current);
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn npy_arrays_are_read() {
        let header = "{'descr': '<f4', 'fortran_order': False, 'shape': (2,), }";
        let mut bytes = b"\x93NUMPY\x01\x00".to_vec();
        bytes.extend((header.len() as u16).to_le_bytes());
        bytes.extend(header.as_bytes());
        bytes.extend(1.5f32.to_le_bytes());
        bytes.extend((-2.0f32).to_le_bytes());
        assert_eq!(parse_npy_f32(&bytes).unwrap(), [1.5, -2.0]);
        assert!(parse_npy_f32(b"nope").is_err());
    }

    #[test]
    fn long_phonemes_are_split_at_spaces() {
        let long = vec!["abcdefghi"; 120].join(" ");
        let parts = split_phonemes(&long);
        assert!(parts.len() > 1);
        assert!(parts.iter().all(|p| p.chars().count() <= MAX_PHONEMES));
        assert_eq!(parts.join(" "), long);
    }

    #[test]
    fn espeak_output_is_tidied() {
        assert_eq!(tidy_phonemes("ˈo_l_a\n ˈm_u_n_d_o_ "), "ˈola ˈmundo");
    }

    /// Needs espeak-ng installed: `cargo test -- --ignored phonemes`.
    #[test]
    #[ignore]
    fn phonemes_match_kokoro_onnx() {
        let mut e = Espeak::load(None).unwrap();
        for (text, lang) in [
            ("Era una tarde tranquila, y la biblioteca olía a papel y a lluvia.", "es"),
            ("¿Dónde estás? ¡Aquí!", "es"),
            ("It was a quiet evening, and the library smelled of paper and rain.", "en-us"),
        ] {
            println!("{text}\n{}\n", phonemize(&mut e, text, lang).unwrap());
        }
    }
}

#[cfg(test)]
mod live {
    use super::*;

    /// Writes a WAV from the downloaded model in $BIBLION_TTS_DIR:
    /// `cargo test -- --ignored speak_to_wav`.
    #[test]
    #[ignore]
    fn speak_to_wav() {
        let dir = PathBuf::from(std::env::var("BIBLION_TTS_DIR").unwrap());
        let tts = Tts { dir: dir.clone(), bundled_espeak: None, espeak: Mutex::new(None), engine: Mutex::new(None) };
        let text = "Era una tarde tranquila, y la biblioteca olía a papel y a lluvia. Entre los estantes, alguien pasó una página.";
        let t = std::time::Instant::now();
        let audio = tts.speak(text, "ef_dora", 0.95).unwrap();
        let first = t.elapsed();
        let t = std::time::Instant::now();
        let again = tts.speak(text, "ef_dora", 0.95).unwrap();
        println!("{:.1}s of audio; first call {:?} (loads the model), then {:?}", audio.len() as f32 / SAMPLE_RATE as f32, first, t.elapsed());
        assert_eq!(audio.len(), again.len());
        let mut wav = Vec::new();
        let data = (audio.len() * 2) as u32;
        wav.extend(b"RIFF"); wav.extend((36 + data).to_le_bytes()); wav.extend(b"WAVEfmt ");
        wav.extend(16u32.to_le_bytes()); wav.extend(1u16.to_le_bytes()); wav.extend(1u16.to_le_bytes());
        wav.extend(SAMPLE_RATE.to_le_bytes()); wav.extend((SAMPLE_RATE * 2).to_le_bytes()); wav.extend(2u16.to_le_bytes()); wav.extend(16u16.to_le_bytes());
        wav.extend(b"data"); wav.extend(data.to_le_bytes());
        for s in audio { wav.extend(((s.clamp(-1.0, 1.0) * 32767.0) as i16).to_le_bytes()); }
        std::fs::write(dir.join("rust_dora.wav"), wav).unwrap();
    }
}
