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

export function useButtonHoverMotion(page: string) {
  useGSAP(() => {
    const media = gsap.matchMedia();
    media.add("(prefers-reduced-motion: no-preference)", () => {
      let hovered: HTMLButtonElement | null = null;
      const tweens = new Map<HTMLButtonElement, gsap.core.Tween>();

      function animate(button: HTMLButtonElement, entering: boolean) {
        tweens.get(button)?.kill();
        const scale = button.classList.contains("project-row") ? 1.004 : 1.025;
        tweens.set(button, gsap.to(button, {
          scale: entering ? scale : 1,
          duration: entering ? 0.15 : 0.18,
          ease: "power2.out",
          clearProps: entering ? undefined : "transform",
          onComplete: () => { tweens.delete(button); },
        }));
      }

      function hover(event: PointerEvent) {
        if (event.pointerType !== "mouse" || !(event.target instanceof Element)) return;
        const button = event.target.closest("button");
        if (!button || (event.relatedTarget instanceof Node && button.contains(event.relatedTarget))) return;

        if (event.type === "pointerout") {
          if (hovered !== button) return;
          hovered = null;
          animate(button, false);
        } else if (!button.disabled && button !== hovered) {
          if (hovered) animate(hovered, false);
          hovered = button;
          animate(button, true);
        }
      }

      document.addEventListener("pointerover", hover);
      document.addEventListener("pointerout", hover);
      return () => {
        document.removeEventListener("pointerover", hover);
        document.removeEventListener("pointerout", hover);
        for (const tween of tweens.values()) tween.kill();
        for (const button of tweens.keys()) gsap.set(button, { clearProps: "transform" });
        if (hovered && !tweens.has(hovered)) gsap.set(hovered, { clearProps: "transform" });
      };
    });
    return () => media.revert();
  }, { dependencies: [page], revertOnUpdate: true });
}
