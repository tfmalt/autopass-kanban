# kanban-web.ps1 - Manage the kanban web UI for ip-2.0
#
# Usage:
#   kanban-web             # start (opens browser)
#   kanban-web start       # start (opens browser)
#   kanban-web stop        # stop running server
#   kanban-web status      # show server state and URL
#   kanban-web restart     # restart (opens browser)
#   kanban-web log         # show recent web log lines
#   kanban-web log -Follow # follow log output
#
# The repo root (ip-2.0) is baked in. No path argument needed.

[CmdletBinding()]
param(
    [Parameter(Position = 0)]
    [ValidateSet('start', 'stop', 'status', 'restart', 'log', 'help')]
    [string]$Command = 'start',

    [Alias('f')]
    [switch]$Follow,

    [ValidateRange(1, 100000)]
    [int]$Lines = 0,

    [switch]$NoOpen,

    [switch]$Foreground
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$RepoRoot = '<PATH TO ..>\ip-2.0'
$KanbanExe = '<PATH TO ..>\kanban.exe'

function Test-KanbanPrerequisites {
    if (-not (Test-Path -LiteralPath $RepoRoot -PathType Container)) {
        throw "Repo root not found: $RepoRoot"
    }

    if (-not (Test-Path -LiteralPath $KanbanExe -PathType Leaf)) {
        throw "kanban.exe not found: $KanbanExe"
    }
}

function Invoke-Kanban {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$Arguments,

        [switch]$SkipPrerequisiteCheck
    )

    if (-not $SkipPrerequisiteCheck) {
        Test-KanbanPrerequisites
    }

    & $KanbanExe @Arguments
    $exitCode = if ($null -eq $LASTEXITCODE) { 0 } else { $LASTEXITCODE }
    exit $exitCode
}

switch ($Command) {
    'start' {
        $argsList = @('web', 'start')
        if (-not $NoOpen) { $argsList += '--open' }
        if ($Foreground) { $argsList += '--foreground' }
        $argsList += $RepoRoot
        Invoke-Kanban -Arguments $argsList
    }

    'stop' {
        Invoke-Kanban -Arguments @('web', 'stop', $RepoRoot)
    }

    'status' {
        Invoke-Kanban -Arguments @('web', 'status', $RepoRoot)
    }

    'restart' {
        $argsList = @('web', 'restart')
        if (-not $NoOpen) { $argsList += '--open' }
        if ($Foreground) { $argsList += '--foreground' }
        $argsList += $RepoRoot
        Invoke-Kanban -Arguments $argsList
    }

    'log' {
        $argsList = @('web', 'log')
        if ($Lines -gt 0) { $argsList += @('--lines', $Lines.ToString()) }
        if ($Follow) { $argsList += '--follow' }
        $argsList += $RepoRoot
        Invoke-Kanban -Arguments $argsList
    }

    'help' {
        Invoke-Kanban -Arguments @('web', '--help') -SkipPrerequisiteCheck
    }
}
