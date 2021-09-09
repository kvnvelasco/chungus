// @ts-nocheck

import { useApplicationState, useEntrypointAnalysis } from "./state";
import * as d3 from "d3";
import { ascending, cluster, curveBundle, hierarchy, lineRadial } from "d3";
import { useEffect, useMemo, useRef, useState } from "react";
import {
  Checkbox,
  Classes,
  Colors,
  FormGroup,
  NumericInput,
  RangeSlider,
  Switch,
} from "@blueprintjs/core";

const PIXEL_RATIO = (function () {
  var ctx = document.createElement("canvas").getContext("2d"),
    dpr = window.devicePixelRatio || 1,
    bsr =
      ctx.webkitBackingStorePixelRatio ||
      ctx.mozBackingStorePixelRatio ||
      ctx.msBackingStorePixelRatio ||
      ctx.oBackingStorePixelRatio ||
      ctx.backingStorePixelRatio ||
      1;

  return dpr / bsr;
})();

export function TreeMap({
  selectedNode,
  selectedPaths,
}: {
  selectedNode: string;
  selectedPaths: Set<string>;
}) {
  const [showTransitiveDependencies, setShowTransitiveDependencies] =
    useState(false);
  const { state } = useApplicationState();
  const [analysis] = useEntrypointAnalysis();
  const [outgoing, setOutgoing] = useState(true);
  const [maxDepth, setMaxDepth] = useState([1, 2]);
  const [transform, setTransform] = useState([
    PIXEL_RATIO,
    0,
    0,
    PIXEL_RATIO,
    0,
    0,
  ]);
  const canvasRef = useRef(null);
  const container = useRef();

  const [{ width, height }, setSize] = useState({
    width: 0,
    height: 0,
  });

  useEffect(() => {
    requestAnimationFrame(() => {
      if (container.current == null) return;
      const { width, height } = container.current.getBoundingClientRect();

      setSize({ width, height });
    });
  }, []);

  const data = useMemo(() => {
    if (analysis != null && selectedPaths.size > 0) {
      let data = {};
      const nodes = analysis.analysis_groups
        .filter((group) => selectedPaths.has(group.full_path))
        .concat(
          analysis.all_nodes.filter((node) => selectedPaths.has(node.full_path))
        );

      if (Object.keys(analysis.chunks).length > 0) {
        data = d3.group(nodes, (value) => {
          return value.chunk ?? -1;
        });
      } else {
        data = { children: nodes };
      }

      const heir = hierarchy(data);

      heir.sort((a, b) => ascending(a.data.full_path, b.data.full_path));
      const radius = d3.max([heir.leaves().length * 10, height / 2]);

      const layout = cluster()
        .size([2 * Math.PI, radius - 100])
        .separation((a, b) => (a.parent === b.parent ? 1 : 2) / a.depth)(heir);

      return { heirarchy: heir, layout, radius };
    }
  }, [selectedPaths, analysis, height]);

  const { edges, depth } = useMemo(() => {
    if (data == null || selectedNode == null) return { edges: [], depth: 1 };
    const seenSet = new Set();
    let deepest = 1;
    let edges = [];
    const leaves = data.layout.leaves();

    const sourceLeaf =
      leaves.find((leaf) => leaf.data.full_path === selectedNode) ||
      leaves.find((leaf) =>
        leaf.data.immediate_children.find(
          (node) => node.full_path === selectedNode
        )
      ) ||
      leaves.find((leaf) =>
        leaf.data.inclusions.find((node) => node.full_path === selectedNode)
      );

    const queue = [[sourceLeaf, 1]];
    function createEdges(sourceLeaf, depth) {
      if (depth > deepest) {
        deepest = depth;
      }
      if (
        seenSet.has(`${sourceLeaf.data.identifier}:${sourceLeaf.data.chunk}`)
      ) {
        return;
      }
      const data = outgoing
        ? sourceLeaf.data.outgoing
        : sourceLeaf.data.incoming;
      data.forEach((pointedLeaf) => {
        // we find each corresponding incoming node
        let target_leaf = leaves.find(
          (targetNode) =>
            targetNode.data.identifier === pointedLeaf.identifier ||
            targetNode.data.inclusions.find(
              (included) => included.identifier === pointedLeaf.identifier
            )
        );

        if (target_leaf == null) {
          return;
        }

        if (
          seenSet.has(
            `${sourceLeaf.data.identifier}:${sourceLeaf.data.chunk}::${target_leaf.data.identifier}:${target_leaf.data.chunk}`
          )
        ) {
          return;
        }

        edges.push([
          outgoing
            ? sourceLeaf.path(target_leaf)
            : target_leaf.path(sourceLeaf),
          depth,
        ]);

        seenSet.add(
          `${sourceLeaf.data.identifier}:${sourceLeaf.data.chunk}::${target_leaf.data.identifier}:${target_leaf.data.chunk}`
        );

        if (showTransitiveDependencies) {
          queue.push([target_leaf, depth + 1]);
        }
      });

      seenSet.add(`${sourceLeaf.data.identifier}:${sourceLeaf.data.chunk}`);
    }

    while (queue.length > 0) {
      const [next, depth] = queue.pop();
      createEdges(next, depth);
    }

    return { edges, depth: deepest };
  }, [data, selectedNode, outgoing, showTransitiveDependencies]);

  useEffect(() => {
    //determine the ratio of height to radius and set the transform accordingly
    if (data?.radius) {
      const radius = data?.radius;
      const heightRadiusRatio = (radius * 2) / height;
      setTransform([
        PIXEL_RATIO / heightRadiusRatio,
        0,
        0,
        PIXEL_RATIO / heightRadiusRatio,
        0,
        0,
      ]);
    }
  }, [data?.radius, height]);

  useEffect(() => {
    if (data != null) {
      const { layout } = data;
      canvasRef.current = d3
        .select("#analysis_map")
        .attr("width", `${width * PIXEL_RATIO}px`)
        .attr("height", `${height * PIXEL_RATIO}px`)
        .style("width", `${width}px`)
        .style("height", `${height}px`)
        .call(
          d3.zoom().on("zoom", function (e) {
            setTransform([
              PIXEL_RATIO * e.transform.k,
              0,
              0,
              PIXEL_RATIO * e.transform.k,
              e.transform.x,
              e.transform.y,
            ]);
          })
        );

      const canvas = canvasRef.current;

      const canvasNode = canvas.node();

      const line = lineRadial()
        .curve(curveBundle.beta(0.83))
        .radius((d: any) => d.y)
        .angle((d: any) => d.x); // Move by just a smidge

      function paint(canvas, transform) {
        const context = canvas.getContext("2d");
        if (context == null) return;
        const fontSize = 14;
        context.font = `${fontSize}px Helvetica`;

        {
          // Reset the canvas
          context.setTransform(PIXEL_RATIO, 0, 0, PIXEL_RATIO, 0, 0);
          context.moveTo(0, 0);
          context.clearRect(0, 0, canvas.width, canvas.height);
          context.moveTo(0, 0);
        }

        context.setTransform(...transform);

        context.translate(width / 2, height / 2);
        context.rotate(-Math.PI / 2);

        layout.each((leaf) => {
          const isChunk = Array.isArray(leaf.data);

          context.save();
          context.rotate(leaf.x - Math.PI / 2);
          context.translate(leaf.y, 0);

          if (!isChunk) {
            if (leaf.data.chunk != null && leaf.data.chunk !== -1) {
              const colors = [
                Colors.COBALT1,
                Colors.BLUE1,
                Colors.INDIGO1,
                Colors.LIME1,
                Colors.GOLD2,
                Colors.VERMILION1,
                Colors.FOREST1,
                Colors.SEPIA1,
              ];
              const color = colors[leaf.data.chunk % 8];
              context.fillStyle = color;
            }

            if (leaf.data.is_node_module) {
              context.fillStyle = Colors.RED3;
            }

            if (selectedNode && selectedNode === leaf.data.full_path) {
              context.font = `bold ${fontSize}px Helvetica`;
              context.fillStyle = Colors.GREEN4;
            } else {
              context.font = `${fontSize}px Helvetica`;
            }

            if (leaf.x <= Math.PI / 2 || leaf.x >= (Math.PI * 3) / 2) {
              context.scale(-1, -1);
              const { width: textSize } = context.measureText(
                leaf.data.resolver_relative_path
              );
              context.translate(-textSize, 0);
            }

            context.textBaseline = "middle";
            context.fillText(leaf.data.resolver_relative_path, 0, 0);
          } else {
            context.rotate(Math.PI / 2);
            context.translate(0, -50);
            context.globalAlpha = 0.5;

            if (leaf.data[0] === -1) {
              const { width: textSize } = context.measureText("Tree shaken");
              context.translate(-textSize / 2, 0);
              context.fillText("Tree shaken", 0, 0);
            }

            const chunk = analysis.chunks[leaf.data[0]];
            if (chunk) {
              const name = chunk.name || chunk.id;
              const { width: textSize } = context.measureText(name);
              context.translate(-textSize / 2, 0);
              context.fillText(name, 0, 0);
            }
          }
          context.restore();
        });


        edges.forEach(([edge, depth]) => {
          if (!(depth >= maxDepth[0] && depth <= maxDepth[1])) {
            return;
          }
          context.strokeStyle = Colors.GREEN4;
          context.beginPath();
          context.globalAlpha = 0.9 / (depth - maxDepth[0]);
          line.context(context)(edge);
          context.stroke();
        });
      }

      paint(canvasNode, transform);
    }
  }, [
    transform,
    analysis,
    state?.active_directory,
    data,
    width,
    height,
    selectedNode,
    showTransitiveDependencies,
    edges,
    maxDepth,
  ]);
  return (
    <div style={{ height: "100%", margin: "auto" }} ref={container}>
      <canvas id="analysis_map" />
      <Checkbox
        style={{ position: "absolute", top: 40, right: 20 }}
        label="Show transitive Dependencies"
        checked={showTransitiveDependencies}
        onChange={() => setShowTransitiveDependencies((show) => !show)}
      />
      <Switch
        style={{ position: "absolute", top: 20, right: 20 }}
        label="Traversal mode"
        innerLabel="Incoming"
        innerLabelChecked="Outgoing"
        checked={outgoing}
        onChange={() => setOutgoing((show) => !show)}
      />
      {showTransitiveDependencies && (
        <FormGroup
          label="Maximum Depth"
          style={{
            position: "absolute",
            bottom: 20,
            right: 20,
            width: depth * 20,
          }}
        >
          <RangeSlider
            min={1}
            max={depth}
            className={Classes.SMALL}
            onChange={(e) => setMaxDepth(e)}
            placeholder="Render Depths"
            value={maxDepth}
          />
        </FormGroup>
      )}
    </div>
  );
}
