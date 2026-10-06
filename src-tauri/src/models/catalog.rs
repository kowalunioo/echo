//! The built-in list of the three 0.1.0 Models (`models.md` rules 1–3). There is no remote
//! catalogue: names, sizes, URLs at pinned revisions and checksums live here.

use serde::{Deserialize, Serialize};
use specta::Type;

/// One of the Models Echo offers. The order of [`ModelId::ALL`] is the list order used in the UI
/// and for every "first downloaded Model in list order" fallback (rules 23 and 26).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ModelId {
    WhisperLargeV3Turbo,
    ParakeetTdt06bV3,
    WhisperSmall,
}

impl ModelId {
    /// Every Model in list order.
    pub const ALL: [ModelId; 3] = [
        ModelId::WhisperLargeV3Turbo,
        ModelId::ParakeetTdt06bV3,
        ModelId::WhisperSmall,
    ];

    /// The position of this Model in [`ModelId::ALL`].
    pub fn index(self) -> usize {
        self as usize
    }

    /// The built-in facts about this Model.
    pub fn info(self) -> &'static ModelInfo {
        match self {
            ModelId::WhisperLargeV3Turbo => &CATALOG[0],
            ModelId::ParakeetTdt06bV3 => &CATALOG[1],
            ModelId::WhisperSmall => &CATALOG[2],
        }
    }
}

/// The built-in facts about one Model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelInfo {
    pub id: ModelId,
    /// The Model's name as shown to the user (a proper name, the same in every UI Language).
    pub name: &'static str,
    /// The file name under the models folder; also the file name in the repository.
    pub file_name: &'static str,
    /// The exact size of the file in bytes.
    pub size: u64,
    /// Lower-case hex SHA-256 of the file.
    pub sha256: &'static str,
    /// The download URL at a pinned revision.
    pub url: &'static str,
    /// How many spoken languages the Model recognises.
    pub languages: u32,
    /// Whether the UI marks it "Recommended" (the default choice).
    pub recommended: bool,
}

/// Builds a download URL in the Hugging Face organisation of the transcribe-cpp authors (ADR
/// 0003). The repositories and revisions were checked against it, and the files against their
/// SHA-256, in #11.
macro_rules! hf_url {
    ($repo:literal, $revision:literal, $file:literal) => {
        concat!(
            "https://huggingface.co/handy-computer/",
            $repo,
            "/resolve/",
            $revision,
            "/",
            $file
        )
    };
}

/// The three Models in list order (rule 1).
pub const CATALOG: [ModelInfo; 3] = [
    ModelInfo {
        id: ModelId::WhisperLargeV3Turbo,
        name: "Whisper large-v3-turbo",
        file_name: "whisper-large-v3-turbo-Q8_0.gguf",
        size: 886_381_760,
        sha256: "b2e30cc286bc9f3aba4db9099fc7403543497c05ce7100d0d83091ddfd25a183",
        url: hf_url!(
            "whisper-large-v3-turbo-gguf",
            "5eaf945c7978e564bae5b28a5b1639dd93c2bfb1",
            "whisper-large-v3-turbo-Q8_0.gguf"
        ),
        languages: 100,
        recommended: true,
    },
    ModelInfo {
        id: ModelId::ParakeetTdt06bV3,
        name: "Parakeet TDT 0.6B v3",
        file_name: "parakeet-tdt-0.6b-v3-Q8_0.gguf",
        size: 739_508_576,
        sha256: "5859f77944efcd8eafa23a6350731960b2b55b2203df51f319665c807d802cc7",
        url: hf_url!(
            "parakeet-tdt-0.6b-v3-gguf",
            "85ac09ea12fc4b1112fa76810059364bc6adc9de",
            "parakeet-tdt-0.6b-v3-Q8_0.gguf"
        ),
        languages: 25,
        recommended: false,
    },
    ModelInfo {
        id: ModelId::WhisperSmall,
        name: "Whisper small",
        file_name: "whisper-small-Q8_0.gguf",
        size: 269_751_136,
        sha256: "9b9c8811bbcc82a7766f0fb0925614bdacb0923b2cc630daeac17108b655b860",
        url: hf_url!(
            "whisper-small-gguf",
            "c0214bd34be9296695486f838e0142f900803159",
            "whisper-small-Q8_0.gguf"
        ),
        languages: 99,
        recommended: false,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    const PUBLISHER: &str = "https://huggingface.co/handy-computer/";

    #[test]
    fn exactly_three_models_in_spec_order_with_whisper_turbo_recommended() {
        let names: Vec<_> = ModelId::ALL.iter().map(|id| id.info().name).collect();
        assert_eq!(
            names,
            [
                "Whisper large-v3-turbo",
                "Parakeet TDT 0.6B v3",
                "Whisper small"
            ]
        );
        let recommended: Vec<_> = CATALOG.iter().filter(|m| m.recommended).collect();
        assert_eq!(recommended.len(), 1);
        assert_eq!(recommended[0].id, ModelId::WhisperLargeV3Turbo);
        for (index, id) in ModelId::ALL.into_iter().enumerate() {
            assert_eq!(id.info().id, id);
            assert_eq!(id.index(), index);
        }
    }

    #[test]
    fn sizes_and_checksums_match_the_spec() {
        assert_eq!(ModelId::WhisperLargeV3Turbo.info().size, 886_381_760);
        assert_eq!(ModelId::ParakeetTdt06bV3.info().size, 739_508_576);
        assert_eq!(ModelId::WhisperSmall.info().size, 269_751_136);
        for model in &CATALOG {
            assert_eq!(model.sha256.len(), 64);
            assert!(model.sha256.chars().all(|c| c.is_ascii_hexdigit()));
        }
    }

    #[test]
    fn urls_point_at_the_publisher_at_a_pinned_revision() {
        for model in &CATALOG {
            assert!(model.url.starts_with(PUBLISHER), "{}", model.url);
            assert!(model.url.ends_with(model.file_name), "{}", model.url);
            let revision = model.url.split("/resolve/").nth(1).unwrap();
            let revision = revision.split('/').next().unwrap();
            assert_eq!(revision.len(), 40, "a full commit hash, not a branch");
        }
    }
}
