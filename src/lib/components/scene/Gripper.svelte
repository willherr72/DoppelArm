<script lang="ts">
  import { T } from '@threlte/core';

  /** Gripper angle in radians. Controls how open the gripper is. */
  export let angle: number = 0;
  export let color: string = '#888888';

  // Map angle to gripper opening (0 = closed, 1 = open)
  $: openAmount = Math.min(1, Math.max(0, (angle - Math.PI * 0.68) / (Math.PI * 0.64)));
  $: spread = openAmount * 0.025; // max spread 25mm per side
</script>

<T.Group>
  <!-- Left finger -->
  <T.Group position.z={spread + 0.008}>
    <T.Mesh>
      <T.BoxGeometry args={[0.04, 0.008, 0.005]} />
      <T.MeshStandardMaterial {color} metalness={0.3} roughness={0.5} />
    </T.Mesh>
  </T.Group>

  <!-- Right finger -->
  <T.Group position.z={-(spread + 0.008)}>
    <T.Mesh>
      <T.BoxGeometry args={[0.04, 0.008, 0.005]} />
      <T.MeshStandardMaterial {color} metalness={0.3} roughness={0.5} />
    </T.Mesh>
  </T.Group>

  <!-- Palm -->
  <T.Mesh position.x={-0.015}>
    <T.BoxGeometry args={[0.01, 0.01, 0.03]} />
    <T.MeshStandardMaterial {color} metalness={0.3} roughness={0.5} />
  </T.Mesh>
</T.Group>
