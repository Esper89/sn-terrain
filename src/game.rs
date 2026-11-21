use std::{path::Path, sync::Arc};

/// A game directory containing a Subnautica game.
#[derive(Debug, Clone)]
pub struct GameDir {
    game: Game,
    path: Arc<Path>,
    terrain_dir: Arc<Path>,
    bepinex_dir: Option<Arc<Path>>,
}

impl GameDir {
    /// Create a game directory from a path.
    ///
    /// This checks to make sure that the game directory and the terrain directory exist, and
    /// auto-detects which game is in the directory by checking the executable name.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, InvalidGameDir> {
        let path: Arc<Path> = path.as_ref().into();
        if !path.is_dir() {
            return Err(InvalidGameDir::GameDirNotFound(DirNotFound::new(path)))
        }

        let mut game = None;
        for g in Game::all() {
            if path.join(g.exe_name()).is_file() && game.replace(*g).is_some() {
                return Err(InvalidGameDir::UnambiguousGameExecutableNotFound(
                    UnambiguousGameExecutableNotFound::new(path)
                ))
            }
        }
        let Some(game) = game else {
            return Err(InvalidGameDir::UnambiguousGameExecutableNotFound(
                UnambiguousGameExecutableNotFound::new(path)
            ))
        };

        let terrain_dir: Arc<Path> = path.join(game.terrain_dir()).into();
        if !terrain_dir.is_dir() {
            return Err(InvalidGameDir::TerrainDirNotFound(DirNotFound::new(path)))
        }

        let bepinex_dir: Arc<Path> = path.join("BepInEx").into();
        let bepinex_dir = if bepinex_dir.is_dir() { Some(bepinex_dir) } else { None };

        Ok(GameDir { game, path, terrain_dir, bepinex_dir })
    }

    /// Which game is in this game directory.
    ///
    /// This game's executable existed in this directory when it was checked.
    pub fn game(&self) -> Game { self.game }

    /// The path to this game directory.
    ///
    /// This path existed and was a directory when this was checked.
    pub fn path(&self) -> &Path { &self.path }

    /// The path to this game's terrain directory, containing `.optoctrees` files.
    ///
    /// This path existed and was a directory when this game directory was checked.
    pub fn terrain_dir(&self) -> &Path { &self.terrain_dir }

    /// The path to this game's [BepInEx] directory, if it exists.
    ///
    /// Returns `None` if the path did not exist or was not a directory when this game directory was
    /// checked.
    ///
    /// [BepInEx]: https://github.com/BepInEx/BepInEx
    pub fn bepinex_dir(&self) -> Option<&Path> { self.bepinex_dir.as_deref() }
}

/// Which Subnautica game.
#[derive(Debug, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Default)]
#[repr(u8)]
pub enum Game {
    /// The original game, [_Subnautica_][sn].
    ///
    /// [sn]: https://en.wikipedia.org/wiki/Subnautica
    #[default] Subnautica = 0,

    /// The spin-off game, [_Subnautica: Below Zero_][bz].
    ///
    /// [bz]: https://en.wikipedia.org/wiki/Subnautica:_Below_Zero
    BelowZero = 1,
}

impl Game {
    /// The relative path to this game's terrain directory from its game directory.
    ///
    /// This directory directly contains `.optoctrees` files. See also
    /// [`data_dir_name`](Game::data_dir_name) and [`streaming_dir_name`](Game::streaming_dir_name).
    pub const fn terrain_dir(self) -> &'static str {
        match self {
            Game::Subnautica => pathlit![
                "Subnautica_Data",
                "StreamingAssets",
                "SNUnmanagedData",
                "Build18",
                "CompiledOctreesCache",
            ],

            Game::BelowZero => pathlit![
                "SubnauticaZero_Data",
                "StreamingAssets",
                "SNUnmanagedData",
                "Expansion",
                "CompiledOctreesCache",
            ],
        }
    }

    /// The name of this game's data directory.
    ///
    /// The data directory is located directly in the game directory. Subnautica's data directory
    /// is named `Subnautica_Data`, while Below Zero's data directory is named
    /// `SubnauticaZero_Data`.
    pub const fn data_dir_name(self) -> &'static str {
        match self {
            Game::Subnautica => "Subnautica_Data",
            Game::BelowZero => "SubnauticaZero_Data",
        }
    }

    /// The name of this game's world streaming directory.
    ///
    /// The world streaming directory is located in `StreamingAssets/SNUnmanagedData` in the game's
    /// [data directory](Game::data_dir_name). Subnautica's world streaming directory is named
    /// `Build18`, while Below Zero's world streaming directory is named `Expansion`.
    pub const fn streaming_dir_name(self) -> &'static str {
        match self {
            Game::Subnautica => "Build18",
            Game::BelowZero => "Expansion",
        }
    }

    /// The name of this game's executable.
    ///
    /// The executable is located directly in the game directory. Subnautica's executable is named
    /// `Subnautica.exe`, while Below Zero's executable is named `SubnauticaZero.exe`.
    pub const fn exe_name(self) -> &'static str {
        match self {
            Game::Subnautica => "Subnautica.exe",
            Game::BelowZero => "SubnauticaZero.exe",
        }
    }

    /// Both games, in order.
    pub const fn all() -> &'static [Game; 2] { &[Game::Subnautica, Game::BelowZero] }
}

use error::*;
pub(super) mod error {
    use super::*;

    /// A game directory was invalid.
    #[derive(Debug, Clone, thiserror::Error)]
    pub enum InvalidGameDir {
        /// The game directory was not found.
        #[error("game directory not found: {0}")]
        GameDirNotFound(DirNotFound),

        /// The game executable was not found, or there were multiple ambiguous game executables.
        #[error("unambiguous game executable not found: {0}")]
        UnambiguousGameExecutableNotFound(UnambiguousGameExecutableNotFound),

        /// The game's terrain directory was not found.
        #[error("terrain directory not found: {0}")]
        TerrainDirNotFound(DirNotFound),
    }

    /// A directory was expected but not found.
    #[derive(Debug, Clone, thiserror::Error)]
    #[error("expected a directory: {}", .path.display())]
    pub struct DirNotFound {
        path: Arc<Path>,
    }

    impl DirNotFound {
        pub(super) fn new(path: Arc<Path>) -> Self {
            DirNotFound { path }
        }

        /// The path that was expected to contain a directory.
        pub fn path(&self) -> &Path { &self.path }
    }

    /// A directory did not contain a game executable, or there were multiple ambiguous game
    /// executables.
    #[derive(Debug, Clone, thiserror::Error)]
    #[error("expected to find an unambiguous game executable in directory: {}", .dir.display())]
    pub struct UnambiguousGameExecutableNotFound {
        dir: Arc<Path>,
    }

    impl UnambiguousGameExecutableNotFound {
        pub(super) fn new(dir: Arc<Path>) -> Self {
            UnambiguousGameExecutableNotFound { dir }
        }

        /// The directory where we expected to find an unambiguous game executable.
        pub fn dir(&self) -> &Path { &self.dir }
    }
}

macro_rules! pathlit {
    ($f:expr $(, $t:expr)* $(,)?) => {
        concat![$f, $(cfg_select! { unix => "/", windows => "\\" }, $t,)*]
    };
}
use pathlit;

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn autodetect() {
        let sn = GameDir::new("tests/cases/game/sn").expect("game dir should open");
        let bz = GameDir::new("tests/cases/game/bz").expect("game dir should open");

        assert_eq!(sn.game(), Game::Subnautica);
        assert_eq!(bz.game(), Game::BelowZero);

        assert!(sn.terrain_dir().join("compiled-batch-0-0-0.optoctrees").is_file());
        assert!(bz.terrain_dir().join("compiled-batch-0-0-0.optoctrees").is_file());

        assert!(sn.bepinex_dir().is_some());
        assert!(bz.bepinex_dir().is_none());
    }
}
