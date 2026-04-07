<script lang="ts">
  import { T } from '@threlte/core';
  import { createEventDispatcher } from 'svelte';

  export let position: [number, number, number] = [0.2, 0.15, 0];
  export let visible: boolean = true;

  const dispatch = createEventDispatcher<{ move: { x: number; y: number; z: number } }>();

  let dragging = false;

  // Simple click-to-place behavior. Full drag requires raycasting
  // which will be refined when the interactivity plugin is configured.
</script>

{#if visible}
  <T.Group position.x={position[0]} position.y={position[1]} position.z={position[2]}>
    <!-- Target sphere -->
    <T.Mesh>
      <T.SphereGeometry args={[0.015, 16, 16]} />
      <T.MeshStandardMaterial
        color="#ff4444"
        emissive="#ff2222"
        emissiveIntensity={0.3}
        transparent
        opacity={0.8}
      />
    </T.Mesh>

    <!-- Crosshair rings -->
    <T.Mesh rotation.x={Math.PI / 2}>
      <T.RingGeometry args={[0.02, 0.022, 32]} />
      <T.MeshBasicMaterial color="#ff4444" transparent opacity={0.5} side={2} />
    </T.Mesh>
    <T.Mesh rotation.y={Math.PI / 2}>
      <T.RingGeometry args={[0.02, 0.022, 32]} />
      <T.MeshBasicMaterial color="#ff4444" transparent opacity={0.5} side={2} />
    </T.Mesh>
  </T.Group>
{/if}
