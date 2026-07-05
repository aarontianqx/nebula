/** 宠物域共享类型。 */

/** 全部 15 个动画状态（资产矩阵见 specs/features/pose-matrix.md）。 */
export type PetState =
  | "idle"
  | "walk"
  | "run"
  | "cheer"
  | "stretch"
  | "petted"
  | "sleep"
  | "rest"
  | "drink"
  | "focus"
  | "tired"
  | "sulk"
  | "greet"
  | "curious"
  | "grabbed";

/** 朝向：1 = 右（素材原始朝向），-1 = 左（渲染端镜像）。 */
export type Facing = 1 | -1;

/** 位移步态。 */
export type Gait = "walk" | "run";
