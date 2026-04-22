<script lang="ts">
  import { T, useLoader } from '@threlte/core';
  import { STLLoader } from 'three/examples/jsm/loaders/STLLoader.js';

  /** Public path to the STL file (relative to /static or absolute URL). */
  export let url: string;
  export let color: string = '#888888';
  /** Scale factor. URDF asset STLs are already in meters. */
  export let scale: number = 1;
  /** Local position offset applied after the mesh is loaded. */
  export let position: [number, number, number] = [0, 0, 0];
  /** Local Euler rotation [x, y, z] in radians, applied after loading. */
  export let rotation: [number, number, number] = [0, 0, 0];
  export let metalness: number = 0.25;
  export let roughness: number = 0.55;

  const loader = useLoader(STLLoader);
  $: geometryStore = loader.load(url);
</script>

{#if $geometryStore}
  <T.Mesh
    geometry={$geometryStore}
    position.x={position[0]}
    position.y={position[1]}
    position.z={position[2]}
    rotation.x={rotation[0]}
    rotation.y={rotation[1]}
    rotation.z={rotation[2]}
    scale.x={scale}
    scale.y={scale}
    scale.z={scale}
    castShadow
    receiveShadow
  >
    <T.MeshStandardMaterial {color} {metalness} {roughness} />
  </T.Mesh>
{/if}
