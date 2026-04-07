<script lang="ts">
  import { T } from '@threlte/core';

  export let color: string = '#888888';
  export let radius: number = 0.012;
</script>

<T.Mesh>
  <T.SphereGeometry args={[radius, 16, 16]} />
  <T.MeshStandardMaterial {color} metalness={0.3} roughness={0.6} />
</T.Mesh>
