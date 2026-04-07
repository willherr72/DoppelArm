<script lang="ts">
  import { T } from '@threlte/core';

  export let length: number = 0.1;
  export let color: string = '#666666';
  export let radius: number = 0.008;

  /**
   * Direction the link extends: 'x' or 'y'.
   * The cylinder geometry is created along Y by default,
   * so we rotate it to align with the desired direction.
   */
  export let direction: 'x' | 'y' = 'x';

  $: halfLength = length / 2;
  $: rotZ = direction === 'x' ? -Math.PI / 2 : 0;
  $: posX = direction === 'x' ? halfLength : 0;
  $: posY = direction === 'y' ? halfLength : 0;
</script>

<T.Group position.x={posX} position.y={posY}>
  <T.Mesh rotation.z={rotZ}>
    <T.CylinderGeometry args={[radius, radius, length, 12]} />
    <T.MeshStandardMaterial {color} metalness={0.2} roughness={0.7} />
  </T.Mesh>
</T.Group>
