// SIM-035: procedural sampled-value functions on one explicit/inferred edge
// clock (IEEE 1800-2009 16.9.3): initial values, $past depth and gating,
// value change functions between edges and a repeated edge in one step.
module tb;
  logic clk = 1'b0;
  logic en = 1'b1;
  logic [3:0] v = 4'h3;
  logic [3:0] u;
  wire [3:0] w;
  assign w = v;

  always @(posedge clk)
    $display("E%0d %h %h", $time, $past(v), $past(v, 1, en));

  initial begin
    u[0] = 1'b1;
    $display("Z %h %h %h %b %b %h", $past(v, 1, , @(posedge clk)),
             $past(u, 1, , @(posedge clk)), $past(w, 1, , @(posedge clk)),
             $rose(v[0], @(posedge clk)), $stable(v, @(posedge clk)), $sampled(u));
    v = 4'h5;
    $display("S %h %h", $sampled(v), v);
    #2 clk = 1'b1;
    #1 begin
      v = 4'h6;
      clk = 1'b0;
    end
    #1 $display("A %h %h %b %b %b %h %b", $past(v, 1, , @(posedge clk)),
                $past(v, 2, , @(posedge clk)), $changed(v, @(posedge clk)),
                $rose(v[0], @(posedge clk)), $fell(v[0], @(posedge clk)), $sampled(v),
                $past(u, 1, , @(posedge clk)));
    #1 begin
      en = 1'b0;
      clk = 1'b1;
    end
    #1 begin
      clk = 1'b0;
      en = 1'b1;
      v = 4'h7;
    end
    #1 clk = 1'b1;
    #1 $display("B %h %h %h %h %h %h %h %b", $past(v, 1, , @(posedge clk)),
                $past(v, 2, , @(posedge clk)), $past(v, 3, , @(posedge clk)),
                $past(v, 4, , @(posedge clk)), $past(v, 1, en, @(posedge clk)),
                $past(v, 2, en, @(posedge clk)), $past(v, 3, en, @(posedge clk)),
                $stable(v, @(posedge clk)));
    #1 begin
      clk = 1'b0;
      clk = 1'b1;
      clk = 1'b0;
      clk = 1'b1;
      v = 4'h8;
    end
    #1 $display("C %h %h %h %b", $past(v, 1, , @(posedge clk)),
                $past(v, 2, , @(posedge clk)), $past(v, 3, , @(posedge clk)),
                $changed(v, @(posedge clk)));
    $finish;
  end
endmodule
