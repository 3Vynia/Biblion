//! Online voices from Microsoft Azure's Speech service, with the user's own
//! key. Off until the user connects a key: only then is the text being read
//! sent to Microsoft, one sentence at a time.

use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Voices offered, with the tuning that makes them read like narrators.
pub struct OnlineVoice {
    pub id: &'static str,
    pub name: &'static str,
    /// The language Biblion files it under ("es", "en-us"…).
    pub lang: &'static str,
    pub azure_name: &'static str,
    pub xml_lang: &'static str,
    /// Prosody: rate in percent and pitch in Hz, relative to the voice.
    pub rate: i32,
    pub pitch: i32,
}

pub const VOICES: &[OnlineVoice] = &[
    OnlineVoice { id: "azure:alvaro", name: "Álvaro (narrator)", lang: "es", azure_name: "es-ES-AlvaroNeural", xml_lang: "es-ES", rate: -15, pitch: -9 },
    OnlineVoice { id: "azure:jorge", name: "Jorge", lang: "es", azure_name: "es-MX-JorgeNeural", xml_lang: "es-MX", rate: -8, pitch: 0 },
];

pub fn voice(id: &str) -> Option<&'static OnlineVoice> {
    VOICES.iter().find(|v| v.id == id)
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AzureKey {
    pub key: String,
    /// Azure region of the resource, e.g. "westeurope".
    pub region: String,
}

impl AzureKey {
    fn host(&self) -> Result<String, String> {
        let region = self.region.trim().to_ascii_lowercase();
        if region.is_empty() || region.len() > 40 || !region.chars().all(|c| c.is_ascii_alphanumeric()) {
            return Err("The region looks wrong: it is a word like “westeurope” or “eastus”".into());
        }
        Ok(format!("https://{region}.tts.speech.microsoft.com"))
    }
}

fn agent() -> ureq::Agent {
    ureq::Agent::config_builder()
        .user_agent("Biblion/0.1 (local library manager)")
        .timeout_global(Some(Duration::from_secs(30)))
        .build()
        .into()
}

fn explain(e: ureq::Error) -> String {
    match e {
        ureq::Error::StatusCode(401 | 403) => "Microsoft did not accept the key, or it belongs to another region".into(),
        ureq::Error::StatusCode(429) => "The free monthly allowance of the Azure voice is used up, or too many requests at once".into(),
        ureq::Error::StatusCode(code) => format!("Microsoft's voice service answered with an error ({code})"),
        e => format!("Could not reach Microsoft's voice service: {e}"),
    }
}

/// Checks a key by listing the voices it can use.
pub fn check(key: &AzureKey) -> Result<(), String> {
    let url = format!("{}/cognitiveservices/voices/list", key.host()?);
    agent().get(&url).header("Ocp-Apim-Subscription-Key", key.key.trim()).call().map_err(explain)?;
    Ok(())
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;").replace('\'', "&apos;")
}

pub fn ssml(v: &OnlineVoice, text: &str, speed: f32) -> String {
    // The reader's pace multiplies the voice's own tuning.
    let rate = ((1.0 + v.rate as f32 / 100.0) * speed - 1.0) * 100.0;
    format!(
        "<speak version=\"1.0\" xmlns=\"http://www.w3.org/2001/10/synthesis\" xml:lang=\"{}\"><voice name=\"{}\"><prosody rate=\"{:+.0}%\" pitch=\"{:+}Hz\">{}</prosody></voice></speak>",
        v.xml_lang,
        v.azure_name,
        rate,
        v.pitch,
        escape(text)
    )
}

/// One sentence of speech: mono float32 samples at 24 kHz.
pub fn speak(key: &AzureKey, v: &OnlineVoice, text: &str, speed: f32) -> Result<Vec<f32>, String> {
    let url = format!("{}/cognitiveservices/v1", key.host()?);
    let mut res = agent()
        .post(&url)
        .header("Ocp-Apim-Subscription-Key", key.key.trim())
        .header("Content-Type", "application/ssml+xml")
        .header("X-Microsoft-OutputFormat", "raw-24khz-16bit-mono-pcm")
        .send(ssml(v, text, speed))
        .map_err(explain)?;
    let pcm = res.body_mut().with_config().limit(32 * 1024 * 1024).read_to_vec().map_err(|e| e.to_string())?;
    Ok(pcm.chunks_exact(2).map(|b| f32::from(i16::from_le_bytes([b[0], b[1]])) / 32768.0).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ssml_carries_the_tuning_and_escapes_text() {
        let alvaro = voice("azure:alvaro").unwrap();
        let s = ssml(alvaro, "Tom & \"Jerry\" <3", 1.0);
        assert!(s.contains("es-ES-AlvaroNeural") && s.contains("rate=\"-15%\"") && s.contains("pitch=\"-9Hz\""), "{s}");
        assert!(s.contains("Tom &amp; &quot;Jerry&quot; &lt;3"));
        // A faster pace on top of the narrator's slower one.
        assert!(ssml(alvaro, "x", 1.2).contains("rate=\"+2%\""));
    }

    #[test]
    fn regions_are_plain_words() {
        let key = |r: &str| AzureKey { key: "k".into(), region: r.into() };
        assert_eq!(key("WestEurope").host().unwrap(), "https://westeurope.tts.speech.microsoft.com");
        for bad in ["", "evil.com/x", "west europe", "a@b"] {
            assert!(key(bad).host().is_err(), "{bad}");
        }
    }
}
