<script lang="ts">
    import { wsStatus, type ConnectionStatus } from "$lib/websocket";

    const statusConfig: Record<ConnectionStatus, { color: string; label: string; pulse: boolean }> = {
        connected: { color: "bg-green-500", label: "Live", pulse: false },
        connecting: { color: "bg-yellow-500", label: "Connecting", pulse: true },
        disconnected: { color: "bg-red-500", label: "Offline", pulse: false },
    };

    let status = $derived($wsStatus);
    let config = $derived(statusConfig[status]);
</script>

<div class="flex items-center gap-2 text-xs text-muted-foreground">
    <div class="relative flex h-2 w-2">
        {#if config.pulse}
            <span
                class="animate-ping absolute inline-flex h-full w-full rounded-full {config.color} opacity-75"
            ></span>
        {/if}
        <span class="relative inline-flex rounded-full h-2 w-2 {config.color}"></span>
    </div>
    <span>{config.label}</span>
</div>
