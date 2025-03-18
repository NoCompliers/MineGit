## MineGit
The project will provide a version control system, similar to Git, for Minecraft worlds.

MineGit operates independently of the game, working directly with Minecraft world files to enable efficient versioning and backups.

The program should allow players to easily back up their worlds and restore them through the CLI. Git or similar existing programs will not be used for this functionality.

During restoration, users will be able to specify specific regions (parts of the world) to be restored. By default, all regions will be restored.

## Requirements
For our project to be successful, it should:
- Not damage the data of the user's world.
- Support committing current changes.
- Provide rollback capabilities to restore previous versions.
- Allow users to specify specific regions to be restored.
- Provide a command-line interface (CLI) for managing world versions.

## Compatibility
Supports: Minecraft: Java Edition 1.12.2+

Not Guaranteed: Compatibility with other Minecraft editions and older versions.

Dependencies
Dependencies listed here might not be used in the final project:

fs_extra (for file copying)
clap (CLI interface)
zstd (file compression)

MineGit will use the Rust programming language.

Through this project, we hope to learn about efficient data storage, Rust-based file operations, and how to handle binary world files effectively.
Also some insights of how git works

## Developed By
This project is being developed by [@ROmanHanushchak](https://github.com/ROmanGanushchak) and [@BRUH1284](https://github.com/BRUH1284) (Vladimir Riazantsev)
