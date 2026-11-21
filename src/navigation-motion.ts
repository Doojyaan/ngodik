import type { RefObject } from "react";
import { useGSAP } from "@gsap/react";
import gsap from "gsap";

gsap.registerPlugin(useGSAP);

export function useNavigationMotion(scope: RefObject<HTMLDivElement | null>, selection: string, selector?: string) {
  useGSAP(() => {
    const root = scope.current;
    const surface = selector ? root?.querySelector(selector) : root;
    if (!surface) return;

    const media = gsap.matchMedia();
    media.add("(prefers-reduced-motion: no-preference)", () => {
      gsap.fromTo(surface, { opacity: 0.8, y: 6 }, {
        opacity: 1,
        y: 0,
        duration: 0.19,
        ease: "power2.out",
        clearProps: "opacity,transform",
      });
    });
    return () => media.revert();
  }, { scope, dependencies: [selection], revertOnUpdate: true });
}
