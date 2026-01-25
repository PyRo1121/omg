import { Component, onMount, onCleanup } from 'solid-js';
import * as THREE from 'three';

const BackgroundMesh: Component = () => {
  let containerRef: HTMLDivElement | undefined;

  onMount(() => {
    if (!containerRef) return;

    // Scene setup
    const scene = new THREE.Scene();
    const camera = new THREE.PerspectiveCamera(
      75,
      window.innerWidth / window.innerHeight,
      0.1,
      1000
    );
    camera.position.z = 20;

    const renderer = new THREE.WebGLRenderer({ alpha: true, antialias: true });
    renderer.setSize(window.innerWidth, window.innerHeight);
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    containerRef.appendChild(renderer.domElement);

    // Geometry
    const geometry = new THREE.PlaneGeometry(60, 60, 64, 64);
    
    // Material
    const material = new THREE.MeshBasicMaterial({
      color: 0x4444ff,
      wireframe: true,
      transparent: true,
      opacity: 0.15,
    });

    const mesh = new THREE.Mesh(geometry, material);
    mesh.rotation.x = -Math.PI / 2.5; // Tilt it back
    scene.add(mesh);

    // Animation variables
    let animationFrameId: number;
    const clock = new THREE.Clock();

    const animate = () => {
      animationFrameId = requestAnimationFrame(animate);
      const elapsedTime = clock.getElapsedTime();

      // Animate vertices
      const positions = geometry.attributes.position.array as Float32Array;
      for (let i = 0; i < positions.length; i += 3) {
        const x = positions[i];
        const y = positions[i + 1];
        
        // Create wave effect
        const wave1 = Math.sin(x * 0.1 + elapsedTime) * 2;
        const wave2 = Math.sin(y * 0.1 + elapsedTime * 0.5) * 2;
        const wave3 = Math.sin((x + y) * 0.05 + elapsedTime * 0.8) * 1.5;
        
        positions[i + 2] = wave1 + wave2 + wave3;
      }
      geometry.attributes.position.needsUpdate = true;

      renderer.render(scene, camera);
    };

    const handleResize = () => {
      camera.aspect = window.innerWidth / window.innerHeight;
      camera.updateProjectionMatrix();
      renderer.setSize(window.innerWidth, window.innerHeight);
    };

    window.addEventListener('resize', handleResize);
    animate();

    onCleanup(() => {
      window.removeEventListener('resize', handleResize);
      cancelAnimationFrame(animationFrameId);
      
      // Cleanup Three.js resources
      geometry.dispose();
      material.dispose();
      renderer.dispose();
      if (containerRef && renderer.domElement) {
        containerRef.removeChild(renderer.domElement);
      }
    });
  });

  return (
    <div
      ref={containerRef}
      class="fixed inset-0 z-[-1] pointer-events-none"
      aria-hidden="true"
    />
  );
};

export default BackgroundMesh;
