<script lang="ts">
  import { Canvas, T } from '@threlte/core';
  import { OrbitControls } from '@threlte/extras';
  import RobotArm from './RobotArm.svelte';
  import GridFloor from './GridFloor.svelte';
  import IKTarget from './IKTarget.svelte';
  import { leaderAngles, followerAngles } from '$lib/stores/joints';
  import { currentMode } from '$lib/stores/app';
  import { ARM_COLORS } from '$lib/utils/arm-config';
</script>

<div class="scene-container">
  <Canvas>
    <!-- Camera -->
    <T.PerspectiveCamera makeDefault position={[0.35, 0.25, 0.35]} fov={50}>
      <OrbitControls
        enableDamping
        target={[0, 0.1, 0]}
      />
    </T.PerspectiveCamera>

    <!-- Lighting -->
    <T.AmbientLight intensity={0.5} />
    <T.DirectionalLight position={[5, 10, 5]} intensity={0.8} castShadow />
    <T.DirectionalLight position={[-3, 5, -3]} intensity={0.3} />

    <!-- Ground grid -->
    <GridFloor />

    <!-- Leader arm (left) -->
    <RobotArm
      angles={$leaderAngles}
      color={ARM_COLORS.leader}
      position={[-0.15, 0, 0]}
    />

    <!-- Follower arm (right) -->
    <RobotArm
      angles={$followerAngles}
      color={ARM_COLORS.follower}
      position={[0.15, 0, 0]}
    />

    <!-- IK target (only in IK mode) -->
    <IKTarget visible={$currentMode === 'ik'} />

    <!-- Axis helper at origin -->
    <T.AxesHelper args={[0.05]} />
  </Canvas>
</div>

<style>
  .scene-container {
    width: 100%;
    height: 100%;
    min-height: 400px;
  }
</style>
