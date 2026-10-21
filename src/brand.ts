// Single source for branding and the "Inspired by" credit (see AGENTS.md).
// To remove the credit everywhere, empty `inspiredBy` in brand.json.
import brand from "../brand.json";
import inspiredLogo from "./assets/inspired-by/granola.svg";

export const BRAND = brand;
export const INSPIRED_BY = brand.inspiredBy?.name ? { ...brand.inspiredBy, logo: inspiredLogo } : null;
