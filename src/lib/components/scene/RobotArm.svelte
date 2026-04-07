<script lang="ts">
  import { T } from '@threlte/core';
  import Joint from './Joint.svelte';
  import Link from './Link.svelte';
  import Gripper from './Gripper.svelte';
  import { LINKS, CENTER_OFFSET } from '$lib/utils/arm-config';

  /** Joint angles in radians (raw converted, 6 values) */
  export let angles: number[] = [];
  /** Arm color */
  export let color: string = '#4a90d9';
  /** World position offset */
  export let position: [number, number, number] = [0, 0, 0];

  // Center-offset the angles so that raw=2048 maps to zero visual rotation
  $: a = angles.map((r) => r - CENTER_OFFSET);

  // Slightly lighter shade for links
  $: linkColor = color;
  $: jointColor = color;
</script>

<T.Group position.x={position[0]} position.y={position[1]} position.z={position[2]}>
  <!-- Base pedestal -->
  <T.Mesh>
    <T.CylinderGeometry args={[0.025, 0.03, 0.015, 16]} />
    <T.MeshStandardMaterial color="#555555" metalness={0.4} roughness={0.5} />
  </T.Mesh>

  <!-- Joint 0: Shoulder Pan (Y rotation) -->
  <T.Group rotation.y={a[0] ?? 0}>
    <Joint color={jointColor} />

    <!-- Base-to-shoulder link (vertical) -->
    <Link length={LINKS.base} color={linkColor} direction="y" />

    <!-- Joint 1: Shoulder Lift (Z rotation) -->
    <T.Group position.y={LINKS.base}>
      <T.Group rotation.z={a[1] ?? 0}>
        <Joint color={jointColor} />

        <!-- Upper arm link (horizontal) -->
        <Link length={LINKS.upperArm} color={linkColor} direction="x" />

        <!-- Joint 2: Elbow Flex (Z rotation) -->
        <T.Group position.x={LINKS.upperArm}>
          <T.Group rotation.z={a[2] ?? 0}>
            <Joint color={jointColor} />

            <!-- Forearm link -->
            <Link length={LINKS.forearm} color={linkColor} direction="x" />

            <!-- Joint 3: Wrist Flex (Z rotation) -->
            <T.Group position.x={LINKS.forearm}>
              <T.Group rotation.z={a[3] ?? 0}>
                <Joint color={jointColor} radius={0.009} />

                <!-- Joint 4: Wrist Roll (X rotation) -->
                <T.Group rotation.x={a[4] ?? 0}>
                  <!-- Wrist-to-EE link -->
                  <Link length={LINKS.wristToEE} color={linkColor} direction="x" radius={0.006} />

                  <!-- Joint 5: Gripper -->
                  <T.Group position.x={LINKS.wristToEE}>
                    <Gripper angle={angles[5] ?? Math.PI} color={jointColor} />
                  </T.Group>
                </T.Group>
              </T.Group>
            </T.Group>
          </T.Group>
        </T.Group>
      </T.Group>
    </T.Group>
  </T.Group>
</T.Group>
