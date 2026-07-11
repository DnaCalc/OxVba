param(
    [string]$ManifestPath = "docs/validation/IDEAL_PROGRAM_MANIFEST_V1.json",
    [string]$IssuesPath = ".beads/issues.jsonl",
    [switch]$SkipReadyQueue,
    [switch]$SkipCycleCheck,
    [string]$RepositoryRoot = ""
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$PSNativeCommandUseErrorActionPreference = $true

$repoRoot = if ([string]::IsNullOrWhiteSpace($RepositoryRoot)) {
    (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
}
else {
    (Resolve-Path $RepositoryRoot).Path
}
. (Join-Path $PSScriptRoot "lib-ideal-program-validation.ps1")

function Assert-ExactSet {
    param(
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][string[]]$Expected,
        [Parameter(Mandatory = $true)][AllowEmptyCollection()][string[]]$Actual,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    $delta = @(
        Compare-Object `
            -ReferenceObject @($Expected | Sort-Object -Unique) `
            -DifferenceObject @($Actual | Sort-Object -Unique)
    )
    if ($delta.Count -gt 0) {
        $details = @($delta | ForEach-Object { "$($_.SideIndicator)$($_.InputObject)" }) -join ", "
        throw "validate-workset-rollout: $Owner differs from the manifest: $details"
    }
}

function Assert-RoutingLabels {
    param(
        [Parameter(Mandatory = $true)]$Issue,
        [Parameter(Mandatory = $true)][string[]]$RequiredLabels,
        [Parameter(Mandatory = $true)][string]$Owner,
        [switch]$AllowMultipleEffects
    )

    $labels = @(Get-IdealIssueLabels -Issue $Issue)
    foreach ($required in $RequiredLabels) {
        if ($labels -notcontains $required) {
            throw "validate-workset-rollout: $Owner is missing label '$required'"
        }
    }

    $effects = @($labels | Where-Object { $_ -in @("delivery", "support") })
    if ($effects.Count -eq 0 -or (-not $AllowMultipleEffects -and $effects.Count -ne 1)) {
        $expectation = if ($AllowMultipleEffects) { "at least one" } else { "exactly one" }
        throw "validate-workset-rollout: $Owner must carry $expectation delivery/support effect label"
    }
    $risks = @($labels | Where-Object { $_ -like "risk-*" })
    if ($risks.Count -ne 1) {
        throw "validate-workset-rollout: $Owner must carry exactly one risk-* label (found $($risks.Count))"
    }
    $tiers = @($labels | Where-Object { $_ -like "tier-*" })
    if ($tiers.Count -ne 1 -or $tiers[0] -notin @("tier-sol", "tier-terra", "tier-luna")) {
        throw "validate-workset-rollout: $Owner must carry exactly one tier-sol/tier-terra/tier-luna label"
    }
    $requiredProfile = @($RequiredLabels | Where-Object { $_ -like "profile-*" })
    $actualProfiles = @($labels | Where-Object { $_ -like "profile-*" })
    if ($requiredProfile.Count -gt 1 -or
        ($requiredProfile.Count -eq 1 -and ($actualProfiles.Count -ne 1 -or $actualProfiles[0] -ne $requiredProfile[0])) -or
        ($requiredProfile.Count -eq 0 -and $actualProfiles.Count -ne 0)) {
        throw "validate-workset-rollout: $Owner must carry exactly the expected profile routing label"
    }
    $requiredEpic = @($RequiredLabels | Where-Object { $_ -like "epic-*" })
    $actualEpics = @($labels | Where-Object { $_ -like "epic-*" })
    if ($requiredEpic.Count -gt 1 -or
        ($requiredEpic.Count -eq 1 -and ($actualEpics.Count -ne 1 -or $actualEpics[0] -ne $requiredEpic[0])) -or
        ($requiredEpic.Count -eq 0 -and $actualEpics.Count -ne 0)) {
        throw "validate-workset-rollout: $Owner must carry exactly the expected execution-epic routing label"
    }
}

function Assert-ExecutableLeafQuality {
    param(
        [Parameter(Mandatory = $true)]$Issue,
        [Parameter(Mandatory = $true)][string[]]$RequiredLabels,
        [Parameter(Mandatory = $true)][string]$Owner
    )

    if ([string]$Issue.issue_type -eq "epic") {
        throw "validate-workset-rollout: $Owner is a leaf but is typed as an epic"
    }
    $estimate = 0
    if ($Issue.PSObject.Properties.Name -contains "estimated_minutes" -and $null -ne $Issue.estimated_minutes) {
        $estimate = [int]$Issue.estimated_minutes
    }
    if ($estimate -lt 1 -or $estimate -gt 480) {
        throw "validate-workset-rollout: $Owner estimate must be 1..480 minutes, found $estimate"
    }

    Assert-RoutingLabels -Issue $Issue -RequiredLabels $RequiredLabels -Owner $Owner

    $labels = @(Get-IdealIssueLabels -Issue $Issue)
    $allowedResources = @(
        "resource-none",
        "resource-rust-writer",
        "resource-cargo-workspace",
        "resource-excel-vbe",
        "resource-registry",
        "resource-vm-provision",
        "resource-large-jit",
        "resource-large-vm3",
        "resource-large-differential",
        "resource-large-rt-abi"
    )
    $resourceLabels = @($labels | Where-Object { $_ -like "resource-*" })
    if ($resourceLabels.Count -eq 0) {
        throw "validate-workset-rollout: $Owner must carry explicit resource-* scheduling metadata"
    }
    foreach ($resource in $resourceLabels) {
        if ($resource -notin $allowedResources) {
            throw "validate-workset-rollout: $Owner has unknown resource label '$resource'"
        }
    }
    if ($resourceLabels -contains "resource-none" -and $resourceLabels.Count -ne 1) {
        throw "validate-workset-rollout: $Owner cannot combine resource-none with a serialized resource"
    }
    $largeResources = @(
        $resourceLabels |
            Where-Object { $_ -in @("resource-large-jit", "resource-large-vm3", "resource-large-differential", "resource-large-rt-abi") }
    )
    if ($largeResources.Count -gt 0 -and $resourceLabels -notcontains "resource-rust-writer") {
        throw "validate-workset-rollout: $Owner with resource-large-* must also carry resource-rust-writer"
    }

    $acceptance = if ($Issue.PSObject.Properties.Name -contains "acceptance_criteria") { [string]$Issue.acceptance_criteria } else { "" }
    $description = if ($Issue.PSObject.Properties.Name -contains "description") { [string]$Issue.description } else { "" }
    if ([string]::IsNullOrWhiteSpace($acceptance)) {
        throw "validate-workset-rollout: $Owner has no acceptance criteria"
    }
    if (-not (Test-IdealContractClauses -Text $description)) {
        throw "validate-workset-rollout: $Owner must name at least one exact contract clause and may not use wildcard clauses"
    }

    $qualityText = "$description`n$acceptance"
    if ($qualityText -notmatch '(?i)\b(test|check|verify|evidence|oracle|matrix|transcript|fixture)\w*\b') {
        throw "validate-workset-rollout: $Owner does not describe its acceptance evidence"
    }
    if ($qualityText -notmatch '(?i)\b(residual|blocker|follow-up|successor|remaining accepted|no accepted residual)\w*\b') {
        throw "validate-workset-rollout: $Owner does not state residual/blocker behavior"
    }
    if ($qualityText -notmatch '(?i)\b(matrix|truth surface|matrix row)\w*\b') {
        throw "validate-workset-rollout: $Owner does not identify a canonical matrix/truth surface"
    }
    Assert-IdealExecutableAcceptanceGrammar -Text $qualityText -Owner $Owner
}

function Get-OwningExpectedEpic {
    param(
        [Parameter(Mandatory = $true)][string]$IssueId,
        [Parameter(Mandatory = $true)][hashtable]$IssueById,
        [Parameter(Mandatory = $true)][hashtable]$ExpectedById
    )

    $owners = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $queue = [Collections.Generic.Queue[string]]::new()
    $queue.Enqueue($IssueId)
    while ($queue.Count -gt 0) {
        $currentId = $queue.Dequeue()
        if (-not $seen.Add($currentId) -or -not $IssueById.ContainsKey($currentId)) {
            continue
        }
        foreach ($parentId in @(Get-IdealParentIds -Issue $IssueById[$currentId])) {
            if ($ExpectedById.ContainsKey($parentId)) {
                [void]$owners.Add($parentId)
            }
            else {
                $queue.Enqueue($parentId)
            }
        }
    }
    if ($owners.Count -ne 1) {
        throw "validate-workset-rollout: leaf $IssueId must have exactly one manifest execution-epic ancestor (found $($owners.Count))"
    }
    return @($owners)[0]
}

function Get-UnresolvedBlockerIds {
    param(
        [Parameter(Mandatory = $true)][string]$IssueId,
        [Parameter(Mandatory = $true)][hashtable]$IssueById
    )

    $blockers = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    $queue = [Collections.Generic.Queue[string]]::new()
    $queue.Enqueue($IssueId)
    while ($queue.Count -gt 0) {
        $currentId = $queue.Dequeue()
        if (-not $seen.Add($currentId) -or -not $IssueById.ContainsKey($currentId)) {
            continue
        }
        $issue = $IssueById[$currentId]
        if ($issue.PSObject.Properties.Name -contains "dependencies" -and $null -ne $issue.dependencies) {
            foreach ($dependency in @($issue.dependencies)) {
                if ([string]$dependency.type -eq "blocks") {
                    $blockerId = [string]$dependency.depends_on_id
                    if ([string]::IsNullOrWhiteSpace($blockerId) -or
                        -not $IssueById.ContainsKey($blockerId) -or
                        [string]$IssueById[$blockerId].status -ne "closed") {
                        [void]$blockers.Add($(if ([string]::IsNullOrWhiteSpace($blockerId)) { "<missing>" } else { $blockerId }))
                    }
                }
            }
        }
        foreach ($parentId in @(Get-IdealParentIds -Issue $issue)) {
            $queue.Enqueue($parentId)
        }
    }
    return @($blockers)
}

function Invoke-BrJson {
    param([Parameter(Mandatory = $true)][string[]]$Arguments)

    $output = @(& br @Arguments --no-auto-flush)
    if ($LASTEXITCODE -ne 0) {
        throw "validate-workset-rollout: br $($Arguments -join ' ') failed with exit code $LASTEXITCODE"
    }
    $text = $output -join "`n"
    if ([string]::IsNullOrWhiteSpace($text)) {
        throw "validate-workset-rollout: br $($Arguments -join ' ') returned no JSON"
    }
    return $text | ConvertFrom-Json
}

Push-Location $repoRoot
try {
    $manifestContext = Read-IdealProgramManifest -RepoRoot $repoRoot -ManifestPath $ManifestPath
    $manifest = $manifestContext.Manifest
    $issueContext = Read-IdealIssues -RepoRoot $repoRoot -IssuesPath $IssuesPath
    $issues = @($issueContext.Issues)
    $issueById = $issueContext.IssueById
    $childrenByParent = New-IdealChildrenMap -Issues $issues
    $expectedEpics = @(Get-IdealExpectedEpicRecords -Manifest $manifest)

    if ($expectedEpics.Count -ne 42) {
        throw "validate-workset-rollout: manifest must define exactly 42 execution epics, found $($expectedEpics.Count)"
    }
    $expectedById = @{}
    foreach ($record in $expectedEpics) {
        if ($expectedById.ContainsKey($record.EpicId)) {
            throw "validate-workset-rollout: manifest repeats execution epic '$($record.EpicId)'"
        }
        $expectedById[$record.EpicId] = $record
    }

    foreach ($requiredId in @([string]$manifest.root_bead, [string]$manifest.control_epic)) {
        if (-not $issueById.ContainsKey($requiredId)) {
            throw "validate-workset-rollout: manifest bead '$requiredId' does not exist"
        }
        if ([string]$issueById[$requiredId].issue_type -ne "epic") {
            throw "validate-workset-rollout: manifest bead '$requiredId' must be an epic"
        }
    }
    $controlIssue = $issueById[[string]$manifest.control_epic]
    $controlDescription = if ($controlIssue.PSObject.Properties.Name -contains "description") { [string]$controlIssue.description } else { "" }
    $controlAcceptance = if ($controlIssue.PSObject.Properties.Name -contains "acceptance_criteria") { [string]$controlIssue.acceptance_criteria } else { "" }
    Assert-IdealExecutableAcceptanceGrammar `
        -Text "$controlDescription`n$controlAcceptance" `
        -Owner "PROGRAM-0 control epic $($manifest.control_epic)"

    $root = $issueById[[string]$manifest.root_bead]
    if (@(Get-IdealParentIds -Issue $root).Count -ne 0) {
        throw "validate-workset-rollout: program root '$($manifest.root_bead)' must not have a parent-child parent"
    }
    if ((Get-IdealIssueLabels -Issue $root) -notcontains [string]$manifest.program_label) {
        throw "validate-workset-rollout: program root is missing label '$($manifest.program_label)'"
    }
    $allProgramDescendants = @(Get-IdealDescendantIds -RootId ([string]$manifest.root_bead) -ChildrenByParent $childrenByParent)
    $programIdSet = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    [void]$programIdSet.Add([string]$manifest.root_bead)
    foreach ($id in $allProgramDescendants) {
        [void]$programIdSet.Add($id)
    }
    foreach ($id in $allProgramDescendants) {
        $parents = @(Get-IdealParentIds -Issue $issueById[$id])
        if ($parents.Count -ne 1 -or -not $programIdSet.Contains($parents[0])) {
            throw "validate-workset-rollout: current-program issue '$id' must have exactly one current-program parent (found '$($parents -join ',')')"
        }
    }
    $rootChildren = if ($childrenByParent.ContainsKey([string]$manifest.root_bead)) { @($childrenByParent[[string]$manifest.root_bead]) } else { @() }
    $expectedRootChildren = @([string]$manifest.control_epic) + @($manifest.profiles | ForEach-Object { [string]$_.workset_root })
    Assert-ExactSet -Expected $expectedRootChildren -Actual @($rootChildren | ForEach-Object { [string]$_.id }) -Owner "program-root direct children"

    foreach ($profile in @($manifest.profiles)) {
        $profileName = [string]$profile.profile
        $profileLabel = switch ($profileName) {
            "core" { "profile-core" }
            "windows-x64" { "profile-win-x64" }
            "ide" { "profile-ide" }
            default { throw "validate-workset-rollout: unknown profile '$profileName'" }
        }
        $worksetRoot = [string]$profile.workset_root
        if (-not $issueById.ContainsKey($worksetRoot)) {
            throw "validate-workset-rollout: missing workset root '$worksetRoot'"
        }
        $worksetIssue = $issueById[$worksetRoot]
        if ([string]$worksetIssue.issue_type -ne "epic") {
            throw "validate-workset-rollout: workset root '$worksetRoot' is not an epic"
        }
        $worksetLabels = @(Get-IdealIssueLabels -Issue $worksetIssue)
        foreach ($label in @([string]$manifest.program_label, $profileLabel, "workset")) {
            if ($worksetLabels -notcontains $label) {
                throw "validate-workset-rollout: workset root '$worksetRoot' is missing label '$label'"
            }
        }
        $actualEpicChildren = if ($childrenByParent.ContainsKey($worksetRoot)) { @($childrenByParent[$worksetRoot]) } else { @() }
        Assert-ExactSet `
            -Expected @($profile.expected_epics | ForEach-Object { [string]$_.bead }) `
            -Actual @($actualEpicChildren | ForEach-Object { [string]$_.id }) `
            -Owner "$profileName workset execution epics"
    }

    foreach ($record in $expectedEpics) {
        if (-not $issueById.ContainsKey($record.EpicId)) {
            throw "validate-workset-rollout: missing execution epic '$($record.EpicId)' ($($record.Code))"
        }
        $epic = $issueById[$record.EpicId]
        if ([string]$epic.issue_type -ne "epic") {
            throw "validate-workset-rollout: '$($record.EpicId)' ($($record.Code)) is not an epic"
        }
        if ([string]$epic.title -notmatch "^$([regex]::Escape($record.Code))(?:\s|:|$)") {
            throw "validate-workset-rollout: '$($record.EpicId)' title does not start with '$($record.Code)'"
        }
        Assert-RoutingLabels `
            -Issue $epic `
            -RequiredLabels @([string]$manifest.program_label, $record.ProfileLabel, $record.EpicLabel) `
            -Owner "execution epic $($record.EpicId)" `
            -AllowMultipleEffects
        $epicLabels = @(Get-IdealIssueLabels -Issue $epic)
        if ($record.Effect -eq "delivery" -and $epicLabels -notcontains "delivery") {
            throw "validate-workset-rollout: execution epic $($record.EpicId) manifest effect delivery requires label 'delivery'"
        }
        if ($record.Effect -eq "support" -and
            ($epicLabels -notcontains "support" -or $epicLabels -contains "delivery")) {
            throw "validate-workset-rollout: execution epic $($record.EpicId) manifest effect support requires support-only labels"
        }

        $description = if ($epic.PSObject.Properties.Name -contains "description") { [string]$epic.description } else { "" }
        $acceptance = if ($epic.PSObject.Properties.Name -contains "acceptance_criteria") { [string]$epic.acceptance_criteria } else { "" }
        if ([string]::IsNullOrWhiteSpace($acceptance) -or -not (Test-IdealContractClauses -Text $description)) {
            throw "validate-workset-rollout: execution epic $($record.EpicId) must carry acceptance criteria and exact, non-wildcard clauses"
        }
        Assert-IdealExecutableAcceptanceGrammar `
            -Text "$description`n$acceptance" `
            -Owner "execution epic $($record.EpicId)"

        $directChildren = if ($childrenByParent.ContainsKey($record.EpicId)) { @($childrenByParent[$record.EpicId]) } else { @() }
        $rollouts = @($directChildren | Where-Object { (Get-IdealIssueLabels -Issue $_) -contains "rollout" })
        if ($rollouts.Count -ne 1) {
            throw "validate-workset-rollout: execution epic $($record.EpicId) must have exactly one direct rollout bead (found $($rollouts.Count))"
        }
        $rollout = $rollouts[0]
        $rolloutLabels = @(Get-IdealIssueLabels -Issue $rollout)
        if ($rolloutLabels -notcontains "support") {
            throw "validate-workset-rollout: rollout $($rollout.id) must be a support bead"
        }
        if ($childrenByParent.ContainsKey([string]$rollout.id) -and @($childrenByParent[[string]$rollout.id]).Count -gt 0) {
            throw "validate-workset-rollout: rollout $($rollout.id) must remain an executable leaf; create successor beads as siblings under the epic"
        }
        Assert-ExecutableLeafQuality `
            -Issue $rollout `
            -RequiredLabels @([string]$manifest.program_label, $record.ProfileLabel, $record.EpicLabel, "rollout", "support") `
            -Owner "rollout bead $($rollout.id)"

        if ([string]$rollout.status -eq "closed") {
            $executableSuccessors = @(
                $directChildren |
                    Where-Object {
                        [string]$_.id -ne [string]$rollout.id -and
                        [string]$_.issue_type -ne "epic"
                    }
            )
            $requiredSuccessors = if ($record.Effect -eq "delivery") {
                @($executableSuccessors | Where-Object { (Get-IdealIssueLabels -Issue $_) -contains "delivery" })
            }
            else {
                @($executableSuccessors | Where-Object { (Get-IdealIssueLabels -Issue $_) -contains "support" })
            }
            if ($requiredSuccessors.Count -eq 0) {
                if ($record.Effect -eq "delivery") {
                    throw "validate-workset-rollout: closed rollout $($rollout.id) has no direct executable delivery successor"
                }
                throw "validate-workset-rollout: closed support-only rollout $($rollout.id) has no direct executable support successor"
            }
        }

        if ([string]$epic.status -eq "closed") {
            $epicDescendants = @(Get-IdealDescendantIds -RootId $record.EpicId -ChildrenByParent $childrenByParent)
            $unfinished = @($epicDescendants | Where-Object { [string]$issueById[$_].status -ne "closed" })
            if ($unfinished.Count -gt 0) {
                throw "validate-workset-rollout: closed execution epic $($record.EpicId) has unfinished descendants: $($unfinished -join ',')"
            }
            $deliveryLeaves = @(
                $epicDescendants |
                    Where-Object {
                        $descendantId = [string]$_
                        [string]$issueById[$descendantId].issue_type -ne "epic" -and
                        (-not $childrenByParent.ContainsKey($descendantId) -or @($childrenByParent[$descendantId]).Count -eq 0) -and
                        (Get-IdealIssueLabels -Issue $issueById[$descendantId]) -contains "delivery"
                    }
            )
            if ($record.Effect -eq "delivery" -and $deliveryLeaves.Count -eq 0) {
                throw "validate-workset-rollout: closed execution epic $($record.EpicId) cannot close on support leaves alone"
            }
            if ($record.Effect -eq "support" -and $epicDescendants.Count -eq 0) {
                throw "validate-workset-rollout: closed support-only execution epic $($record.EpicId) has no executable support outcome"
            }
        }
    }

    $executionDescendants = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
    foreach ($profile in @($manifest.profiles)) {
        foreach ($id in @(Get-IdealDescendantIds -RootId ([string]$profile.workset_root) -ChildrenByParent $childrenByParent)) {
            [void]$executionDescendants.Add($id)
        }
    }
    foreach ($id in @($executionDescendants)) {
        $issue = $issueById[$id]
        $hasChildren = $childrenByParent.ContainsKey($id) -and @($childrenByParent[$id]).Count -gt 0
        if ([string]$issue.issue_type -eq "epic" -and -not $expectedById.ContainsKey($id)) {
            throw "validate-workset-rollout: unmanifested nested execution epic '$id' is not allowed"
        }
        if ($hasChildren -or [string]$issue.issue_type -eq "epic") {
            continue
        }
        $ownerEpicId = Get-OwningExpectedEpic -IssueId $id -IssueById $issueById -ExpectedById $expectedById
        $ownerRecord = $expectedById[$ownerEpicId]
        Assert-ExecutableLeafQuality `
            -Issue $issue `
            -RequiredLabels @([string]$manifest.program_label, $ownerRecord.ProfileLabel, $ownerRecord.EpicLabel) `
            -Owner "execution leaf $id"
    }

    $activeExecutionLeaves = @(
        $executionDescendants |
            Where-Object {
                $candidate = $issueById[[string]$_]
                [string]$candidate.status -eq "in_progress" -and
                [string]$candidate.issue_type -ne "epic" -and
                (-not $childrenByParent.ContainsKey([string]$candidate.id) -or @($childrenByParent[[string]$candidate.id]).Count -eq 0)
            } |
            ForEach-Object { $issueById[[string]$_] }
    )
    foreach ($activeLeaf in $activeExecutionLeaves) {
        $unresolvedBlockers = @(Get-UnresolvedBlockerIds -IssueId ([string]$activeLeaf.id) -IssueById $issueById)
        if ($unresolvedBlockers.Count -gt 0) {
            throw "validate-workset-rollout: active executable leaf $($activeLeaf.id) has unresolved blocker(s), including ancestor blockers: $($unresolvedBlockers -join ',')"
        }
    }
    if ($activeExecutionLeaves.Count -gt 3) {
        throw "validate-workset-rollout: active executable leaves exceed three-worker limit 3: $(@($activeExecutionLeaves | ForEach-Object { [string]$_.id }) -join ',')"
    }
    $resourceLimits = @(
        @{ Name = "Rust writers"; Maximum = 2; Match = { param($labels) $labels -contains "resource-rust-writer" } },
        @{ Name = "workspace Cargo gates"; Maximum = 1; Match = { param($labels) $labels -contains "resource-cargo-workspace" } },
        @{ Name = "Excel/VBE automation lanes"; Maximum = 1; Match = { param($labels) $labels -contains "resource-excel-vbe" } },
        @{ Name = "registry mutation lanes"; Maximum = 1; Match = { param($labels) $labels -contains "resource-registry" } },
        @{ Name = "certification-VM provisioning lanes"; Maximum = 1; Match = { param($labels) $labels -contains "resource-vm-provision" } },
        @{ Name = "large JIT/VM3/differential/rt-abi writers"; Maximum = 1; Match = {
            param($labels)
            @($labels | Where-Object { $_ -in @("resource-large-jit", "resource-large-vm3", "resource-large-differential", "resource-large-rt-abi") }).Count -gt 0
        } }
    )
    foreach ($limit in $resourceLimits) {
        $owners = @(
            $activeExecutionLeaves |
                Where-Object { & $limit.Match @(Get-IdealIssueLabels -Issue $_) } |
                ForEach-Object { [string]$_.id }
        )
        if ($owners.Count -gt [int]$limit.Maximum) {
            throw "validate-workset-rollout: active $($limit.Name) exceed limit $($limit.Maximum): $($owners -join ',')"
        }
    }

    $controlDescendants = @(Get-IdealDescendantIds -RootId ([string]$manifest.control_epic) -ChildrenByParent $childrenByParent)
    foreach ($id in $controlDescendants) {
        $issue = $issueById[$id]
        $hasChildren = $childrenByParent.ContainsKey($id) -and @($childrenByParent[$id]).Count -gt 0
        if ($hasChildren -or [string]$issue.issue_type -eq "epic") {
            continue
        }
        Assert-RoutingLabels `
            -Issue $issue `
            -RequiredLabels @([string]$manifest.program_label, "program-0") `
            -Owner "PROGRAM-0 leaf $id"
        $estimate = if ($issue.PSObject.Properties.Name -contains "estimated_minutes") { [int]$issue.estimated_minutes } else { 0 }
        if ($estimate -lt 1 -or $estimate -gt 480) {
            throw "validate-workset-rollout: PROGRAM-0 leaf $id estimate must be 1..480 minutes, found $estimate"
        }
        $acceptance = if ($issue.PSObject.Properties.Name -contains "acceptance_criteria") { [string]$issue.acceptance_criteria } else { "" }
        if ([string]::IsNullOrWhiteSpace($acceptance)) {
            throw "validate-workset-rollout: PROGRAM-0 leaf $id has no acceptance criteria"
        }
        $description = if ($issue.PSObject.Properties.Name -contains "description") { [string]$issue.description } else { "" }
        Assert-IdealExecutableAcceptanceGrammar `
            -Text "$description`n$acceptance" `
            -Owner "PROGRAM-0 leaf $id"
    }

    if (-not $SkipCycleCheck) {
        $cycleResult = Invoke-BrJson -Arguments @("dep", "cycles", "--json")
        if ([int]$cycleResult.count -ne 0 -or @($cycleResult.cycles).Count -ne 0) {
            throw "validate-workset-rollout: dependency graph contains $($cycleResult.count) cycle(s)"
        }
    }

    foreach ($profile in @($manifest.profiles)) {
        $worksetId = [string]$profile.workset_root
        if ([string]$issueById[$worksetId].status -eq "closed") {
            $unfinished = @(Get-IdealDescendantIds -RootId $worksetId -ChildrenByParent $childrenByParent | Where-Object { [string]$issueById[$_].status -ne "closed" })
            if ($unfinished.Count -gt 0) {
                throw "validate-workset-rollout: closed workset root $worksetId has unfinished descendants: $($unfinished -join ',')"
            }
        }
    }
    if ([string]$root.status -eq "closed") {
        $unfinished = @(Get-IdealDescendantIds -RootId ([string]$manifest.root_bead) -ChildrenByParent $childrenByParent | Where-Object { [string]$issueById[$_].status -ne "closed" })
        if ($unfinished.Count -gt 0) {
            throw "validate-workset-rollout: closed program root $($manifest.root_bead) has unfinished descendants: $($unfinished -join ',')"
        }
    }

    $readyCount = -1
    if (-not $SkipReadyQueue) {
        $globalReady = @(Invoke-BrJson -Arguments @("ready", "--limit", "0", "--json"))
        $programReady = @(Invoke-BrJson -Arguments @("ready", "--parent", [string]$manifest.root_bead, "--recursive", "--limit", "0", "--json"))
        $globalIds = @($globalReady | ForEach-Object { [string]$_.id })
        $programIds = @($programReady | ForEach-Object { [string]$_.id })
        Assert-ExactSet -Expected $programIds -Actual $globalIds -Owner "global ready queue versus current-program ready leaves"
        foreach ($ready in $globalReady) {
            $readyId = [string]$ready.id
            if (-not $issueById.ContainsKey($readyId)) {
                throw "validate-workset-rollout: ready issue '$readyId' is absent from $IssuesPath"
            }
            $fullIssue = $issueById[$readyId]
            if ([string]$fullIssue.issue_type -eq "epic") {
                throw "validate-workset-rollout: ready queue contains epic '$readyId'"
            }
            if ((Get-IdealIssueLabels -Issue $fullIssue) -notcontains [string]$manifest.program_label) {
                throw "validate-workset-rollout: ready queue contains stale/non-program issue '$readyId'"
            }
            if ($childrenByParent.ContainsKey($readyId) -and @($childrenByParent[$readyId]).Count -gt 0) {
                throw "validate-workset-rollout: ready issue '$readyId' is not a leaf"
            }
        }
        $readyCount = $globalReady.Count
        if ([string]$root.status -ne "closed" -and ($readyCount + $activeExecutionLeaves.Count) -eq 0) {
            throw "validate-workset-rollout: open program has no ready or active executable leaf"
        }
    }

    $queueText = if ($SkipReadyQueue) { "skipped" } else { "$readyCount active=$($activeExecutionLeaves.Count)" }
    Write-Host "validate-workset-rollout: ok (program=$($manifest.program_id) profiles=3 epics=$($expectedEpics.Count) rollouts=$($expectedEpics.Count) ready=$queueText)"
}
finally {
    Pop-Location
}
