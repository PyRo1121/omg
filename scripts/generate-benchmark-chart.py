#!/usr/bin/env python3
"""
Generate benchmark comparison chart for OMG documentation.

This script creates a visual comparison of OMG performance vs traditional
package managers (pacman, yay, apt-cache, Nala).

Usage:
    python3 scripts/generate-benchmark-chart.py

Output:
    docs/assets/benchmark-comparison.png
    docs/assets/benchmark-comparison-apt.png
"""

import matplotlib.pyplot as plt
import numpy as np
from pathlib import Path


def create_arch_benchmark_chart():
    """Create benchmark chart for Arch Linux (pacman/yay)."""
    
    # Data from README.md benchmarks
    categories = ['Search', 'Info', 'Status', 'Explicit']
    omg = [6, 6.5, 7, 1.2]
    pacman = [133, 138, None, 14]  # Status not available
    yay = [150, 300, None, 27]

    # Set up the bar chart
    x = np.arange(len(categories))
    width = 0.25

    fig, ax = plt.subplots(figsize=(12, 7))
    
    # Create bars
    bars1 = ax.bar(x - width, omg, width, label='OMG (Daemon)', 
                   color='#4CAF50', edgecolor='black', linewidth=1.2)
    bars2 = ax.bar(x, [p if p else 0 for p in pacman], width, 
                   label='pacman', color='#FF9800', edgecolor='black', linewidth=1.2)
    bars3 = ax.bar(x + width, [y if y else 0 for y in yay], width, 
                   label='yay', color='#F44336', edgecolor='black', linewidth=1.2)

    # Customize the chart
    ax.set_ylabel('Time (milliseconds)', fontsize=13, fontweight='bold')
    ax.set_title('OMG Performance vs Traditional Package Managers (Arch Linux)', 
                 fontsize=16, fontweight='bold', pad=20)
    ax.set_xticks(x)
    ax.set_xticklabels(categories, fontsize=12)
    ax.legend(fontsize=11, loc='upper left')
    ax.set_ylim(0, 320)
    ax.grid(axis='y', alpha=0.3, linestyle='--')
    
    # Add value labels on bars
    for bars in [bars1, bars2, bars3]:
        for bar in bars:
            height = bar.get_height()
            if height > 0:
                ax.text(bar.get_x() + bar.get_width()/2., height,
                       f'{height:.1f}ms',
                       ha='center', va='bottom', fontsize=9, fontweight='bold')
    
    # Add speedup annotations
    speedups = [22, 21, None, 12]  # From README
    for i, speedup in enumerate(speedups):
        if speedup:
            ax.text(x[i], 290, f'{speedup}x faster',
                   ha='center', fontsize=10, fontweight='bold',
                   bbox=dict(boxstyle='round', facecolor='#4CAF50', alpha=0.7, edgecolor='black'))
    
    # Add benchmark environment info
    env_text = ("Benchmark Environment:\n"
                "CPU: Intel i9-14900K (32 cores, 5.8GHz)\n"
                "RAM: 31GB | Kernel: Linux 6.18.3\n"
                "Iterations: 10 (with 2 warmup runs)")
    ax.text(0.98, 0.97, env_text, transform=ax.transAxes,
           fontsize=8, verticalalignment='top', horizontalalignment='right',
           bbox=dict(boxstyle='round', facecolor='wheat', alpha=0.5))
    
    plt.tight_layout()
    
    # Save the figure
    output_dir = Path(__file__).parent.parent / 'docs' / 'assets'
    output_dir.mkdir(parents=True, exist_ok=True)
    output_path = output_dir / 'benchmark-comparison.png'
    
    plt.savefig(output_path, dpi=300, bbox_inches='tight', facecolor='white')
    print(f"✓ Arch Linux benchmark chart saved to: {output_path}")
    
    return output_path


def create_debian_benchmark_chart():
    """Create benchmark chart for Debian/Ubuntu (apt-cache/Nala)."""
    
    # Data from README.md benchmarks
    categories = ['Search', 'Info', 'Explicit']
    omg = [11, 27, 2]
    apt_cache = [652, 462, 601]
    nala = [1160, 788, 966]

    # Set up the bar chart
    x = np.arange(len(categories))
    width = 0.25

    fig, ax = plt.subplots(figsize=(12, 7))
    
    # Create bars
    bars1 = ax.bar(x - width, omg, width, label='OMG (Daemon)', 
                   color='#4CAF50', edgecolor='black', linewidth=1.2)
    bars2 = ax.bar(x, apt_cache, width, 
                   label='apt-cache', color='#FF9800', edgecolor='black', linewidth=1.2)
    bars3 = ax.bar(x + width, nala, width, 
                   label='Nala', color='#9C27B0', edgecolor='black', linewidth=1.2)

    # Customize the chart
    ax.set_ylabel('Time (milliseconds)', fontsize=13, fontweight='bold')
    ax.set_title('OMG Performance vs APT Tools (Debian/Ubuntu)', 
                 fontsize=16, fontweight='bold', pad=20)
    ax.set_xticks(x)
    ax.set_xticklabels(categories, fontsize=12)
    ax.legend(fontsize=11, loc='upper left')
    ax.set_ylim(0, 1250)
    ax.grid(axis='y', alpha=0.3, linestyle='--')
    
    # Add value labels on bars
    for bars in [bars1, bars2, bars3]:
        for bar in bars:
            height = bar.get_height()
            if height > 0:
                ax.text(bar.get_x() + bar.get_width()/2., height,
                       f'{int(height)}ms',
                       ha='center', va='bottom', fontsize=9, fontweight='bold')
    
    # Add speedup annotations
    speedups_apt = [59, 17, 300]  # vs apt-cache
    speedups_nala = [105, 29, 483]  # vs Nala
    
    for i, (speedup_apt, speedup_nala) in enumerate(zip(speedups_apt, speedups_nala)):
        # Show maximum speedup
        max_speedup = max(speedup_apt, speedup_nala)
        ax.text(x[i], 1150, f'{max_speedup}x faster',
               ha='center', fontsize=10, fontweight='bold',
               bbox=dict(boxstyle='round', facecolor='#4CAF50', alpha=0.7, edgecolor='black'))
    
    # Add benchmark environment info
    env_text = ("Benchmark Environment:\n"
                "OS: Ubuntu 24.04 (Docker)\n"
                "Iterations: 5 (with 2 warmup runs)")
    ax.text(0.98, 0.97, env_text, transform=ax.transAxes,
           fontsize=8, verticalalignment='top', horizontalalignment='right',
           bbox=dict(boxstyle='round', facecolor='wheat', alpha=0.5))
    
    plt.tight_layout()
    
    # Save the figure
    output_dir = Path(__file__).parent.parent / 'docs' / 'assets'
    output_dir.mkdir(parents=True, exist_ok=True)
    output_path = output_dir / 'benchmark-comparison-apt.png'
    
    plt.savefig(output_path, dpi=300, bbox_inches='tight', facecolor='white')
    print(f"✓ Debian/Ubuntu benchmark chart saved to: {output_path}")
    
    return output_path


def create_combined_speedup_chart():
    """Create a combined speedup comparison chart."""
    
    fig, (ax1, ax2) = plt.subplots(1, 2, figsize=(16, 6))
    
    # Arch Linux speedups
    categories_arch = ['Search', 'Info', 'Explicit']
    speedups_arch = [22, 21, 12]
    
    bars1 = ax1.barh(categories_arch, speedups_arch, color='#4CAF50', 
                     edgecolor='black', linewidth=1.2)
    ax1.set_xlabel('Speedup (x times faster)', fontsize=12, fontweight='bold')
    ax1.set_title('Arch Linux: OMG vs pacman/yay', fontsize=14, fontweight='bold')
    ax1.grid(axis='x', alpha=0.3, linestyle='--')
    
    # Add value labels
    for bar in bars1:
        width = bar.get_width()
        ax1.text(width, bar.get_y() + bar.get_height()/2.,
                f'{int(width)}x',
                ha='left', va='center', fontsize=11, fontweight='bold',
                bbox=dict(boxstyle='round', facecolor='white', alpha=0.8))
    
    # Debian/Ubuntu speedups (maximum between apt-cache and Nala)
    categories_debian = ['Search', 'Info', 'Explicit']
    speedups_debian = [105, 29, 483]  # Max speedups vs Nala
    
    bars2 = ax2.barh(categories_debian, speedups_debian, color='#FF5722',
                     edgecolor='black', linewidth=1.2)
    ax2.set_xlabel('Speedup (x times faster)', fontsize=12, fontweight='bold')
    ax2.set_title('Debian/Ubuntu: OMG vs apt-cache/Nala', fontsize=14, fontweight='bold')
    ax2.grid(axis='x', alpha=0.3, linestyle='--')
    
    # Add value labels
    for bar in bars2:
        width = bar.get_width()
        ax2.text(width, bar.get_y() + bar.get_height()/2.,
                f'{int(width)}x',
                ha='left', va='center', fontsize=11, fontweight='bold',
                bbox=dict(boxstyle='round', facecolor='white', alpha=0.8))
    
    fig.suptitle('OMG Speedup Comparison Across Platforms', 
                 fontsize=16, fontweight='bold', y=0.98)
    
    plt.tight_layout()
    
    # Save the figure
    output_dir = Path(__file__).parent.parent / 'docs' / 'assets'
    output_dir.mkdir(parents=True, exist_ok=True)
    output_path = output_dir / 'benchmark-speedup.png'
    
    plt.savefig(output_path, dpi=300, bbox_inches='tight', facecolor='white')
    print(f"✓ Combined speedup chart saved to: {output_path}")
    
    return output_path


def main():
    """Generate all benchmark charts."""
    print("Generating benchmark comparison charts...\n")
    
    try:
        # Generate charts
        arch_chart = create_arch_benchmark_chart()
        debian_chart = create_debian_benchmark_chart()
        speedup_chart = create_combined_speedup_chart()
        
        print("\n✅ All benchmark charts generated successfully!")
        print("\nGenerated files:")
        print(f"  1. {arch_chart}")
        print(f"  2. {debian_chart}")
        print(f"  3. {speedup_chart}")
        
        print("\nNext steps:")
        print("  - Add charts to README.md after benchmark tables")
        print("  - Optimize images: optipng docs/assets/*.png")
        print("  - Commit with: git add docs/assets/*.png")
        
    except Exception as e:
        print(f"❌ Error generating charts: {e}")
        return 1
    
    return 0


if __name__ == '__main__':
    exit(main())
