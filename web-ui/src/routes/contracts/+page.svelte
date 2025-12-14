<script lang="ts">
    import { onMount } from "svelte";
    import {
        listContracts,
        deployContract,
        callContract,
        type ContractInfo,
        type CallResponse,
    } from "$lib/api";
    import * as Card from "$lib/components/ui/card";
    import { Button } from "$lib/components/ui/button";
    import { Input } from "$lib/components/ui/input";
    import { Label } from "$lib/components/ui/label";
    import { Badge } from "$lib/components/ui/badge";
    import { Separator } from "$lib/components/ui/separator";
    import * as Tabs from "$lib/components/ui/tabs";

    let contracts = $state<ContractInfo[]>([]);
    let selectedContract = $state<ContractInfo | null>(null);
    let loading = $state(true);

    // Deploy state
    let sourceCode = $state(`; Simple addition contract
; Returns the sum of two arguments
ARG 0
ARG 1
ADD
RETURN`);
    let deploying = $state(false);
    let deployResult = $state<{ address: string; code_size: number } | null>(
        null,
    );
    let deployError = $state("");

    // Call state
    let callArgs = $state("10, 25");
    let calling = $state(false);
    let callResult = $state<CallResponse | null>(null);
    let callError = $state("");

    onMount(async () => {
        await loadContracts();
    });

    async function loadContracts() {
        loading = true;
        try {
            contracts = await listContracts();
            if (contracts.length > 0 && !selectedContract) {
                selectedContract = contracts[0];
            }
        } catch (e) {
            console.error(e);
        } finally {
            loading = false;
        }
    }

    async function handleDeploy() {
        if (!sourceCode.trim()) return;
        deploying = true;
        deployError = "";
        deployResult = null;

        try {
            deployResult = await deployContract(sourceCode);
            await loadContracts();
        } catch (e: any) {
            deployError = e.message;
        } finally {
            deploying = false;
        }
    }

    async function handleCall() {
        if (!selectedContract) return;
        calling = true;
        callError = "";
        callResult = null;

        try {
            const args = callArgs
                .split(",")
                .map((s) => parseInt(s.trim()))
                .filter((n) => !isNaN(n));
            callResult = await callContract(selectedContract.address, args);
        } catch (e: any) {
            callError = e.message;
        } finally {
            calling = false;
        }
    }

    function selectContract(contract: ContractInfo) {
        selectedContract = contract;
        callResult = null;
        callError = "";
    }
</script>

<svelte:head>
    <title>Contracts | Mini-Blockchain</title>
</svelte:head>

<div class="flex flex-col gap-6">
    <div class="flex flex-col gap-1">
        <h1 class="text-3xl font-bold tracking-tight">Smart Contracts</h1>
        <p class="text-muted-foreground">
            Deploy and interact with smart contracts
        </p>
    </div>

    <Tabs.Root value="deploy" class="w-full">
        <Tabs.List class="grid w-full grid-cols-3">
            <Tabs.Trigger value="deploy">Deploy</Tabs.Trigger>
            <Tabs.Trigger value="contracts"
                >Contracts ({contracts.length})</Tabs.Trigger
            >
            <Tabs.Trigger value="call">Call</Tabs.Trigger>
        </Tabs.List>

        <!-- Deploy Tab -->
        <Tabs.Content value="deploy" class="mt-4">
            <div class="grid gap-6 lg:grid-cols-2">
                <Card.Root>
                    <Card.Header>
                        <Card.Title>📜 Deploy Contract</Card.Title>
                        <Card.Description
                            >Paste assembly code to deploy a new contract</Card.Description
                        >
                    </Card.Header>
                    <Card.Content class="space-y-4">
                        <div class="space-y-2">
                            <Label for="source"
                                >Contract Source (Assembly)</Label
                            >
                            <textarea
                                id="source"
                                bind:value={sourceCode}
                                class="w-full h-48 p-3 rounded-lg bg-muted font-mono text-sm border border-input resize-none"
                                placeholder="; Your contract code here..."
                            ></textarea>
                        </div>
                        {#if deployError}
                            <p class="text-sm text-destructive">
                                {deployError}
                            </p>
                        {/if}
                    </Card.Content>
                    <Card.Footer>
                        <Button
                            class="w-full"
                            onclick={() => handleDeploy()}
                            disabled={deploying || !sourceCode.trim()}
                        >
                            {deploying ? "Deploying..." : "🚀 Deploy Contract"}
                        </Button>
                    </Card.Footer>
                </Card.Root>

                <Card.Root>
                    <Card.Header>
                        <Card.Title>Deployment Result</Card.Title>
                        <Card.Description
                            >Status of your deployment</Card.Description
                        >
                    </Card.Header>
                    <Card.Content>
                        {#if deployResult}
                            <div class="space-y-6">
                                <div
                                    class="rounded-lg border border-green-500/50 bg-green-500/10 p-4"
                                >
                                    <p
                                        class="text-green-500 font-medium flex items-center gap-2"
                                    >
                                        <span>✅</span> Contract Deployed!
                                    </p>
                                </div>
                                <div class="space-y-2">
                                    <p class="text-sm text-muted-foreground">
                                        Contract Address
                                    </p>
                                    <code
                                        class="block text-xs font-mono break-all bg-muted p-3 rounded-lg"
                                        >{deployResult.address}</code
                                    >
                                </div>
                                <div class="flex gap-4">
                                    <div class="space-y-1">
                                        <p
                                            class="text-sm text-muted-foreground"
                                        >
                                            Code Size
                                        </p>
                                        <p class="text-lg font-bold">
                                            {deployResult.code_size} bytes
                                        </p>
                                    </div>
                                </div>
                            </div>
                        {:else}
                            <div
                                class="flex flex-col items-center justify-center h-48 text-muted-foreground"
                            >
                                <span class="text-4xl mb-4">📜</span>
                                <p class="font-medium">No deployment yet</p>
                                <p class="text-sm">
                                    Deploy a contract to see results
                                </p>
                            </div>
                        {/if}
                    </Card.Content>
                </Card.Root>
            </div>
        </Tabs.Content>

        <!-- Contracts List Tab -->
        <Tabs.Content value="contracts" class="mt-4">
            <Card.Root>
                <Card.Header>
                    <div class="flex items-center justify-between">
                        <div>
                            <Card.Title>Deployed Contracts</Card.Title>
                            <Card.Description
                                >{contracts.length} contract(s) on chain</Card.Description
                            >
                        </div>
                        <Button
                            variant="outline"
                            size="sm"
                            onclick={() => loadContracts()}>🔄 Refresh</Button
                        >
                    </div>
                </Card.Header>
                <Card.Content>
                    {#if loading}
                        <div class="flex items-center justify-center h-32">
                            <div
                                class="h-6 w-6 animate-spin rounded-full border-2 border-primary border-t-transparent"
                            ></div>
                        </div>
                    {:else if contracts.length === 0}
                        <div
                            class="flex flex-col items-center justify-center h-48 text-muted-foreground"
                        >
                            <span class="text-4xl mb-4">📜</span>
                            <p class="font-medium">No contracts deployed</p>
                            <p class="text-sm">
                                Deploy a contract using the Deploy tab
                            </p>
                        </div>
                    {:else}
                        <div class="space-y-4">
                            {#each contracts as contract, i}
                                {#if i > 0}
                                    <Separator />
                                {/if}
                                <button
                                    type="button"
                                    class="w-full text-left p-4 rounded-lg border transition-colors hover:bg-accent cursor-pointer
                    {selectedContract?.address === contract.address
                                        ? 'bg-accent border-primary'
                                        : 'border-transparent'}"
                                    onclick={() => selectContract(contract)}
                                >
                                    <div
                                        class="flex items-center justify-between"
                                    >
                                        <div class="space-y-1">
                                            <p class="font-mono text-sm">
                                                {contract.address}
                                            </p>
                                            <p
                                                class="text-xs text-muted-foreground"
                                            >
                                                Deployer: {contract.deployer}
                                            </p>
                                        </div>
                                        <div class="flex items-center gap-2">
                                            <Badge variant="outline"
                                                >{contract.code_size} bytes</Badge
                                            >
                                            <Badge variant="secondary"
                                                >Block #{contract.deployed_at}</Badge
                                            >
                                        </div>
                                    </div>
                                </button>
                            {/each}
                        </div>
                    {/if}
                </Card.Content>
            </Card.Root>
        </Tabs.Content>

        <!-- Call Tab -->
        <Tabs.Content value="call" class="mt-4">
            <div class="grid gap-6 lg:grid-cols-2">
                <Card.Root>
                    <Card.Header>
                        <Card.Title>📞 Call Contract</Card.Title>
                        <Card.Description
                            >Execute a contract with arguments</Card.Description
                        >
                    </Card.Header>
                    <Card.Content class="space-y-4">
                        <div class="space-y-2">
                            <Label>Selected Contract</Label>
                            {#if selectedContract}
                                <code
                                    class="block text-xs font-mono break-all bg-muted p-3 rounded-lg"
                                    >{selectedContract.address}</code
                                >
                            {:else}
                                <p class="text-sm text-muted-foreground">
                                    Select a contract from the Contracts tab
                                </p>
                            {/if}
                        </div>

                        <div class="space-y-2">
                            <Label for="args"
                                >Arguments (comma-separated numbers)</Label
                            >
                            <Input
                                id="args"
                                bind:value={callArgs}
                                placeholder="10, 25"
                            />
                        </div>

                        {#if callError}
                            <p class="text-sm text-destructive">{callError}</p>
                        {/if}
                    </Card.Content>
                    <Card.Footer>
                        <Button
                            class="w-full"
                            onclick={() => handleCall()}
                            disabled={calling || !selectedContract}
                        >
                            {calling ? "Calling..." : "▶️ Execute Contract"}
                        </Button>
                    </Card.Footer>
                </Card.Root>

                <Card.Root>
                    <Card.Header>
                        <Card.Title>Execution Result</Card.Title>
                        <Card.Description>Contract call output</Card.Description
                        >
                    </Card.Header>
                    <Card.Content>
                        {#if callResult}
                            <div class="space-y-6">
                                <div
                                    class="rounded-lg border {callResult.success
                                        ? 'border-green-500/50 bg-green-500/10'
                                        : 'border-destructive/50 bg-destructive/10'} p-4"
                                >
                                    <p
                                        class="{callResult.success
                                            ? 'text-green-500'
                                            : 'text-destructive'} font-medium flex items-center gap-2"
                                    >
                                        <span
                                            >{callResult.success
                                                ? "✅"
                                                : "❌"}</span
                                        >
                                        {callResult.success
                                            ? "Execution Successful"
                                            : "Execution Failed"}
                                    </p>
                                </div>

                                <div class="grid grid-cols-2 gap-4">
                                    <div class="space-y-1">
                                        <p
                                            class="text-sm text-muted-foreground"
                                        >
                                            Return Value
                                        </p>
                                        <p class="text-2xl font-bold">
                                            {callResult.return_value ?? "None"}
                                        </p>
                                    </div>
                                    <div class="space-y-1">
                                        <p
                                            class="text-sm text-muted-foreground"
                                        >
                                            Gas Used
                                        </p>
                                        <p class="text-2xl font-bold font-mono">
                                            {callResult.gas_used}
                                        </p>
                                    </div>
                                </div>
                            </div>
                        {:else}
                            <div
                                class="flex flex-col items-center justify-center h-48 text-muted-foreground"
                            >
                                <span class="text-4xl mb-4">📜</span>
                                <p class="font-medium">No result yet</p>
                                <p class="text-sm">
                                    Call a contract to see results
                                </p>
                            </div>
                        {/if}
                    </Card.Content>
                </Card.Root>
            </div>
        </Tabs.Content>
    </Tabs.Root>
</div>
