import React, { useEffect, useRef } from "react";
import cytoscape from "cytoscape";
import { MemoryGraph } from "../hooks/useMemoryGraph";

interface MemoryGraphViewProps {
  graph: MemoryGraph;
  onNodeSelect: (nodeId: string) => void;
}

const NODE_COLORS: Record<string, string> = {
  module: "#7aa2f7",
  bug: "#f7768e",
  decision: "#e0af68",
  invariant: "#bb9af7",
  task: "#9ece6a",
};

export const MemoryGraphView: React.FC<MemoryGraphViewProps> = ({
  graph,
  onNodeSelect,
}) => {
  const containerRef = useRef<HTMLDivElement>(null);
  const cyRef = useRef<cytoscape.Core | null>(null);

  useEffect(() => {
    if (!containerRef.current) return;

    const elements: cytoscape.ElementDefinition[] = [];

    // Add nodes
    for (const node of graph.nodes) {
      const label = node.id.replace("N.", "");
      elements.push({
        data: {
          id: node.id,
          label,
          type: node.node_type,
          l0: node.l0,
          l1: node.l1,
          score: node.score,
        },
      });
    }

    // Add edges
    for (let i = 0; i < graph.edges.length; i++) {
      const edge = graph.edges[i];
      elements.push({
        data: {
          id: `e${i}`,
          source: edge.from,
          target: edge.to,
          label: edge.rel,
        },
      });
    }

    if (cyRef.current) {
      cyRef.current.destroy();
    }

    const cy = cytoscape({
      container: containerRef.current,
      elements,
      style: [
        {
          selector: "node",
          style: {
            label: "data(label)",
            "background-color": (ele: any) =>
              NODE_COLORS[ele.data("type")] || "#565a6e",
            color: "#a9b1d6",
            "font-size": "11px",
            "text-valign": "bottom",
            "text-margin-y": 6,
            width: (ele: any) => 20 + ele.data("score") * 20,
            height: (ele: any) => 20 + ele.data("score") * 20,
            "border-width": 2,
            "border-color": "#292e42",
          } as any,
        },
        {
          selector: "node:selected",
          style: {
            "border-color": "#c0caf5",
            "border-width": 3,
          },
        },
        {
          selector: "edge",
          style: {
            label: "data(label)",
            "line-color": "#292e42",
            "target-arrow-color": "#565a6e",
            "target-arrow-shape": "triangle",
            "curve-style": "bezier",
            "font-size": "9px",
            color: "#565a6e",
            "text-rotation": "autorotate",
            "text-margin-y": -8,
            width: 1.5,
          } as any,
        },
      ],
      layout: {
        name: "cose",
        animate: false,
        padding: 20,
        nodeRepulsion: () => 8000,
        idealEdgeLength: () => 80,
        nodeOverlap: 20,
      } as any,
      userZoomingEnabled: true,
      userPanningEnabled: true,
      boxSelectionEnabled: false,
    });

    cy.on("tap", "node", (event) => {
      const nodeId = event.target.id();
      onNodeSelect(nodeId);
    });

    cyRef.current = cy;

    return () => {
      cy.destroy();
    };
  }, [graph, onNodeSelect]);

  return <div ref={containerRef} className="memory-graph-container" />;
};
