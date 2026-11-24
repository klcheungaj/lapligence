// FIN-002 regression: releases in a design whose continuous drivers repeat
// in many instances. The instances of `leaf`, the iterations of the generate
// loop and the port connections each share one generated driver body; every
// release must repeat the driver of its own instance or element with that
// instance's operands, parameter and index. See readme.md.
`timescale 1ns / 1ns
module leaf #(
    parameter logic [7:0] K = 8'd1
) (
    input  logic [7:0] d,
    output logic [7:0] q
);
  logic [7:0] twice;
  assign q = d + K;
  assign twice = {d[6:0], 1'b0};
endmodule

module tb;
  logic [7:0] in[6];
  logic [7:0] out[6];
  logic [7:0] direct[6];
  for (genvar i = 0; i < 6; i++) begin : g
    localparam logic [7:0] INDEX = i;
    leaf #(.K(i < 3 ? 8'd1 : 8'd11)) u (
        .d(in[i]),
        .q(out[i])
    );
    assign direct[i] = in[i] + INDEX;
  end

  task show(input string label);
    $display("%s: q=%0d,%0d,%0d,%0d,%0d,%0d twice=%0d,%0d,%0d,%0d out=%0d,%0d direct=%0d,%0d", label,
             g[0].u.q, g[1].u.q, g[2].u.q, g[3].u.q, g[4].u.q, g[5].u.q, g[0].u.twice,
             g[1].u.twice, g[4].u.twice, g[5].u.twice, out[1], out[5], direct[2], direct[5]);
  endtask

  initial begin
    for (int unsigned i = 0; i < 6; i++) in[i] = 8'(20 + i);
    #1 show("driven");
    force g[0].u.q = 8'd200;
    force g[1].u.q = 8'd201;
    force g[2].u.q = 8'd202;
    force g[3].u.q = 8'd203;
    force g[4].u.q = 8'd204;
    force g[5].u.q = 8'd205;
    force g[0].u.twice = 8'd210;
    force g[1].u.twice = 8'd211;
    force g[4].u.twice = 8'd214;
    force g[5].u.twice = 8'd215;
    force out[1] = 8'd221;
    force out[5] = 8'd225;
    force direct[2] = 8'd232;
    force direct[5] = 8'd235;
    #1 for (int unsigned i = 0; i < 6; i++) in[i] = 8'(40 + 2 * i);
    #1 show("forced");
    // Each release names one instance; the others keep their forces.
    release g[4].u.q;
    $display("g[4].u.q at its release: %0d", g[4].u.q);
    release g[1].u.twice;
    $display("g[1].u.twice at its release: %0d", g[1].u.twice);
    release direct[5];
    $display("direct[5] at its release: %0d", direct[5]);
    release out[5];
    $display("out[5] at its release: %0d", out[5]);
    #1 show("four released");
    release g[0].u.q;
    release g[1].u.q;
    release g[2].u.q;
    release g[3].u.q;
    release g[5].u.q;
    release g[0].u.twice;
    release g[4].u.twice;
    release g[5].u.twice;
    release out[1];
    release direct[2];
    $display("at the releases: q=%0d,%0d,%0d,%0d,%0d twice=%0d,%0d,%0d out=%0d direct=%0d",
             g[0].u.q, g[1].u.q, g[2].u.q, g[3].u.q, g[5].u.q, g[0].u.twice, g[4].u.twice,
             g[5].u.twice, out[1], direct[2]);
    #1 show("all released");
    $finish;
  end
endmodule
