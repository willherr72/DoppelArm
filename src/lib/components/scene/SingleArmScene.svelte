<script lang="ts">
  import { Canvas, T } from '@threlte/core';
  import { OrbitControls } from '@threlte/extras';
  import RobotArm from './RobotArm.svelte';
  import GridFloor from './GridFloor.svelte';

  export let angles: number[] = [];
  export let color: string = '#4a90d9';
</script>

<div class="scene-container">
  <Canvas>
    <T.PerspectiveCamera makeDefault position={[0.25, 0.2, 0.25]} fov={50}>
      <OrbitControls
        enableDamping
        target={[0, 0.08, 0]}
      />
    </T.PerspectiveCamera>

    <T.AmbientLight intensity={0.5} />
    <T.DirectionalLight position={[5, 10, 5]} intensity={0.8} />
    <T.DirectionalLight position={[-3, 5, -3]} intensity={0.3} />

    <GridFloor />

    <RobotArm
      {angles}
      {color}
      position={[0, 0, 0]}
    />

    <T.AxesHelper args={[0.03]} />
  </Canvas>
</div>

<style>
  .scene-container {
    width: 100%;
    height: 100%;
    min-height: 200px;
    border-radius: 6px;
    overflow: hidden;
    background: #0a0e17;
  }
</style>
