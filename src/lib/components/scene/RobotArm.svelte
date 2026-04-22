<script lang="ts">
  import { T } from '@threlte/core';
  import StlMesh from './StlMesh.svelte';

  /** Joint angles in radians (calibrated). angles[5] = gripper open [0..1]. */
  export let angles: number[] = [];
  /** Color for the printed plastic parts. */
  export let color: string = '#4a90d9';
  /** World position offset for the whole arm. */
  export let position: [number, number, number] = [0, 0, 0];

  $: a = angles;

  // Color used for the servos (matches URDF "sts3215" material — dark plastic).
  const SERVO_COLOR = '#1a1a1a';
  const ASSET = '/so-arm100/';

  // Constant baseline translation so the arm sits centered on the grid.
  // Tune in meters: +Z = scene blue axis (toward camera).
  const BASE_OFFSET: [number, number, number] = [0, 0, 0];

  // Gripper jaw rotation endpoints (radians). URDF idealized limits are
  // -0.17 to +1.75 rad, but the real mechanical range is much smaller.
  // Tune these two until the jaws touch when closed and open to the
  // desired max angle without flying off.
  const GRIPPER_CLOSED_ANGLE = 0.55;
  const GRIPPER_OPEN_ANGLE = -0.7;
  $: gripperJointAngle =
    GRIPPER_CLOSED_ANGLE + (a[5] ?? 0) * (GRIPPER_OPEN_ANGLE - GRIPPER_CLOSED_ANGLE);
</script>

<T.Group
  position.x={position[0] + BASE_OFFSET[0]}
  position.y={position[1] + BASE_OFFSET[1]}
  position.z={position[2] + BASE_OFFSET[2]}
  rotation.x={-Math.PI / 2}
  rotation.z={Math.PI / 2}
>
  <!-- ===== base_link (static) — visuals wrapped in a 90° fix so the plate lies flat ===== -->
  <T.Group rotation.z={Math.PI / 2}>
    <T.Group
      position={[-0.00636471, -9.94414e-05, -0.0024]}
      rotation={[1.5708, 0, 1.5708]}
    >
      <StlMesh url={ASSET + 'base_motor_holder_so101_v1.stl'} {color} />
    </T.Group>
    <T.Group
      rotation.x={-Math.PI / 2}
      rotation.y={-Math.PI / 2}
      rotation.z={Math.PI / 2}
    >
      <T.Group
        position={[-0.00636471, 0, -0.0024]}
        rotation={[1.5708, 0, 1.5708]}
      >
        <StlMesh url={ASSET + 'base_so101_v2.stl'} {color} />
      </T.Group>
    </T.Group>
    <T.Group position={[0.0263353, 0, 0.0437]} rotation={[0, 0, 0]}>
      <StlMesh url={ASSET + 'sts3215_03a_v1.stl'} color={SERVO_COLOR} />
    </T.Group>
  </T.Group>

  <!-- ===== shoulder_pan joint → shoulder_link ===== -->
  <T.Group
    position={[0.0388353, 0, 0.0624]}
    rotation={[3.14159, 0, -3.14159]}
  >
    <T.Group rotation.z={a[0] ?? 0}>
      <T.Group
        position={[-0.0303992, 0.000422241, -0.0417]}
        rotation={[1.5708, 1.5708, 0]}
      >
        <StlMesh url={ASSET + 'sts3215_03a_v1.stl'} color={SERVO_COLOR} />
      </T.Group>
      <T.Group
        position={[-0.0675992, -0.000177759, 0.0158499]}
        rotation={[1.5708, -1.5708, 0]}
      >
        <StlMesh url={ASSET + 'motor_holder_so101_base_v1.stl'} {color} />
      </T.Group>
      <T.Group
        position={[0.0122008, 2.22413e-05, 0.0464]}
        rotation={[-1.5708, 0, 0]}
      >
        <StlMesh url={ASSET + 'rotation_pitch_so101_v1.stl'} {color} />
      </T.Group>

      <!-- ===== shoulder_lift joint → upper_arm_link ===== -->
      <T.Group
        position={[-0.0303992, -0.0182778, -0.0542]}
        rotation={[-1.5708, -1.5708, 0]}
      >
        <T.Group rotation.z={a[1] ?? 0}>
          <!-- Visuals only: 180° flip (user reported shoulder_lift mesh backwards) -->
          <T.Group rotation.x={Math.PI}>
            <T.Group
              position={[-0.11257, -0.0155, 0.0187]}
              rotation={[-3.14159, 0, -1.5708]}
            >
              <StlMesh url={ASSET + 'sts3215_03a_v1.stl'} color={SERVO_COLOR} />
            </T.Group>
            <T.Group
              position={[-0.065085, 0.012, 0.0182]}
              rotation={[3.14159, 0, 0]}
            >
              <StlMesh url={ASSET + 'upper_arm_so101_v1.stl'} {color} />
            </T.Group>
          </T.Group>

          <!-- ===== elbow_flex joint → lower_arm_link ===== -->
          <T.Group
            position={[-0.11257, -0.028, 0]}
            rotation={[0, 0, 1.5708]}
          >
            <T.Group rotation.z={a[2] ?? 0}>
              <T.Group
                position={[-0.0648499, -0.032, 0.0182]}
                rotation={[3.14159, 0, 0]}
              >
                <StlMesh url={ASSET + 'under_arm_so101_v1.stl'} {color} />
              </T.Group>
              <T.Group
                position={[-0.0648499, -0.032, 0.018]}
                rotation={[-3.14159, 0, 0]}
              >
                <StlMesh url={ASSET + 'motor_holder_so101_wrist_v1.stl'} {color} />
              </T.Group>
              <T.Group
                position={[-0.1224, 0.0052, 0.0187]}
                rotation={[-3.14159, 0, -3.14159]}
              >
                <StlMesh url={ASSET + 'sts3215_03a_v1.stl'} color={SERVO_COLOR} />
              </T.Group>

              <!-- ===== wrist_flex joint → wrist_link ===== -->
              <T.Group
                position={[-0.1349, 0.0052, 0]}
                rotation={[0, 0, -1.5708]}
              >
                <T.Group rotation.z={a[3] ?? 0}>
                  <!-- Translation + mirror wrapper for motor + connected body -->
                  <T.Group position={[0, 0, 0.037]} rotation.y={Math.PI}>
                    <T.Group rotation.y={Math.PI / 2} rotation.z={Math.PI / 2}>
                      <T.Group
                        position={[-0.045, 0.008, 0.002]}
                        rotation={[1.5708, 1.5708, 0]}
                      >
                        <StlMesh
                          url={ASSET + 'sts3215_03a_no_horn_v1.stl'}
                          color={SERVO_COLOR}
                        />
                      </T.Group>
                    </T.Group>
                    <T.Group rotation.y={Math.PI / 2} rotation.z={-Math.PI / 2}>
                      <T.Group
                        position={[0.028, -0.02, 0.0]}
                        rotation={[-1.5708, -1.5708, 0]}
                      >
                        <StlMesh
                          url={ASSET + 'wrist_roll_pitch_so101_v2.stl'}
                          {color}
                        />
                      </T.Group>
                    </T.Group>
                  </T.Group>

                  <!-- ===== wrist_roll joint → gripper_link ===== -->
                  <T.Group
                    position={[0, -0.0611, 0.0181]}
                    rotation={[1.5708, 0.0486795, 3.14159]}
                  >
                    <T.Group rotation.z={a[4] ?? 0}>
                      <!-- Visuals only: 180° flip (user reported wrist_roll mesh backwards) -->
                      <T.Group rotation.x={Math.PI}>
                        <T.Group
                          position={[0.0077, 0.0001, -0.0234]}
                          rotation={[-1.5708, 0, 0]}
                        >
                          <StlMesh
                            url={ASSET + 'sts3215_03a_v1.stl'}
                            color={SERVO_COLOR}
                          />
                        </T.Group>
                        <T.Group
                          position={[0, -0.000218214, 0.000949706]}
                          rotation={[-3.14159, 0, 0]}
                        >
                          <StlMesh
                            url={ASSET + 'wrist_roll_follower_so101_v1.stl'}
                            {color}
                          />
                        </T.Group>
                      </T.Group>

                      <!-- ===== gripper joint → moving_jaw_link ===== -->
                      <T.Group
                        position={[0.0202, 0.0188, -0.0234]}
                        rotation={[1.5708, 0, 0]}
                      >
                        <!-- Pivot shift: translate BEFORE the joint rotation so
                             the rotation axis moves with the mesh. This keeps
                             the jaw hinging on itself rather than sweeping. -->
                        <T.Group position={[0.00, 0.045, 0.038]}>
                          <T.Group rotation.z={gripperJointAngle}>
                            <!-- Visuals only: 180° flip (user reported jaw inverted) -->
                            <T.Group rotation.x={Math.PI}>
                              <T.Group position={[0, 0, 0.0189]} rotation={[0, 0, 0]}>
                                <StlMesh
                                  url={ASSET + 'moving_jaw_so101_v1.stl'}
                                  {color}
                                />
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
          </T.Group>
        </T.Group>
      </T.Group>
    </T.Group>
  </T.Group>
</T.Group>
