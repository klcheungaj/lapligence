// Allocation failure matrix: covergroups with value, transition and cross
// bins, their queries, a coverage database written at the end of the run,
// and concurrent assertions with their attempt records.
`timescale 1ns / 1ns
module tb;
  logic clk = 1'b0;
  logic [3:0] state = '0;
  logic [7:0] data = '0;
  logic valid = 1'b0;
  int passes, fails;

  covergroup traffic @(posedge clk);
    option.per_instance = 1;
    states: coverpoint state {
      bins low = {[0 : 3]};
      bins mid[4] = {[4 : 11]};
      bins high = {[12 : 15]};
    }
    steps: coverpoint state {
      bins up = (0 => 1 => 2);
      bins wrap = (15 => 0);
      bins never = (3 => 9);
    }
    values: coverpoint data {
      bins ranges[8] = {[0 : 255]};
    }
    both: cross states, values;
  endgroup

  class sampler;
    covergroup by_call with function sample (int kind, bit flag);
      option.per_instance = 1;
      kinds: coverpoint kind {bins each[] = {[0 : 5]};}
      flags: coverpoint flag;
      pairs: cross kinds, flags;
    endgroup
    function new();
      by_call = new;
    endfunction
  endclass

  traffic t = new;
  sampler s = new;

  always #5 clk = ~clk;
  always @(posedge clk) begin
    state <= state + 4'd1;
    data  <= data + 8'd37;
    valid <= state[0];
  end

  property follows;
    @(posedge clk) valid |=> !valid ##1 valid;
  endproperty
  checked :
  assert property (follows) passes++;
  else fails++;
  seen :
  cover property (@(posedge clk) state == 4'd7 ##2 state == 4'd9);

  initial begin
    $set_coverage_db_name("alloc_coverage.db");
    for (int i = 0; i < 12; i++) s.by_call.sample(i % 6, i[1]);
    repeat (20) @(posedge clk);
    $display("traffic %0.2f %0.2f %0.2f", t.states.get_coverage(), t.steps.get_coverage(), t.both.get_coverage());
    $display("calls %0.2f %0.2f", s.by_call.get_inst_coverage(), $get_coverage());
    $display("assertions %0d %0d", passes > 0, fails);
    $display("done");
    $finish;
  end
endmodule
