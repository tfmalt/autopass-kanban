# Tasks for US-001

Parent User Story: US-001
Sprint: ~

## TASK-US-001-001 - Scaffold install.sh

Status: done
Tags: 

Description:
Scaffold scripts/install.sh with flag parsing (--binary, --prefix, --dry-run, --quiet), usage message, and --dry-run helpers

## TASK-US-001-002 - Binary copy and chmod

Status: done
Tags: 

Description:
Implement binary copy to PREFIX/kanban and chmod +x

## TASK-US-001-003 - Shell rc detection and PATH append

Status: done
Tags: 

Description:
Implement shell detection (bash/zsh/ash/fish) and sentinel-guarded PATH export append to rc files

## TASK-US-001-004 - Bash and zsh completion install

Status: done
Tags: 

Description:
Implement bash completion (BASH_COMPLETION_USER_DIR) and zsh completion (fpath, compinit guard) install

## TASK-US-001-005 - Manifest write

Status: done
Tags: 

Description:
Implement install manifest.txt write at PREFIX/lib/kanban/manifest.txt with tab-separated records

## TASK-US-001-006 - Integration test fixture

Status: done
Tags: 

Description:
Add tests/install/ integration test with stub kanban binary and test script

## TASK-US-001-007 - Document install command

Status: done
Tags: 

Description:
Document sh scripts/install.sh --binary <path> with --prefix, --dry-run, --quiet flags in README and HOWTO.md
