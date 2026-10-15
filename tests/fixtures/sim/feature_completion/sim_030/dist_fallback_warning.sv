// llg-test-fixture: tests/fixtures/sim/feature_completion/sim_030/dist_fallback_warning.sv
// SIM-030 follow-up (S30-D9): when a component exceeds the exact solver budget, the fallback solvers keep the constraints but not the dist weights and solve...before orders; llg warns once per call site.
class Weighted;
  rand bit [7:0] x;
  rand bit [7:0] y;
  constraint c { x dist { 0 := 100, [1:255] := 1 }; y > x; solve y before x; }
endclass
class Plain;
  rand bit [7:0] p;
  constraint c { p > 10; }
endclass
class Sized;
  rand bit [3:0] a[];
  constraint c { a.size == 3; foreach (a[i]) a[i] dist { 0 := 5, [1:15] := 1 }; }
endclass
module tb;
  Weighted w = new;
  Plain pl = new;
  Sized sz = new;
  int bad;
  initial begin
    repeat (4) begin
      if (!w.randomize() || w.y <= w.x) bad++;
    end
    repeat (4) begin
      if (!pl.randomize() || pl.p <= 10) bad++;
    end
    repeat (3) begin
      if (!sz.randomize() || sz.a.size() != 3) bad++;
    end
    $display("bad=%0d", bad);
  end
endmodule
