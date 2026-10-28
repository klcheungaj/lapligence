// Semantic-token requirements ST-70 to ST-75 (docs/semantic_tokens.md):
// names in assertions, sequence/property/let declarations and event
// controls. `an_dut` and `an_bus_if` are declared in assertion_names_dut.sv,
// another project file, so the open-buffer stream needs that unit (ST-75).
module an_top(input logic tck, input logic rst_n,
              input logic flash1_csn, input logic wire1, input logic a,
              input logic b);
  logic q;
  logic [1:0] sel;
  an_bus_if bus();
  an_dut dut(.clk(tck), .sel(sel));
  // ST-70: a concurrent assertion with hierarchical names.
  assert property (@(posedge tck) dut.bridge_mode && dut.intreg_flash_select==0 |-> flash1_csn);
  // ST-71, ST-72: named sequences, properties and lets.
  sequence an_seq(x, int n = 1);
    @(posedge tck) x ##n b;
  endsequence
  property an_prop(y);
    @(posedge tck) disable iff (!rst_n) $rose(y) |=> $past(a) && an_seq(y);
  endproperty
  property an_unused(z);
    @(posedge tck) z |-> dut.bridge_mode;
  endproperty
  let an_and(l, r) = l && r;
  an_a: assert property (an_prop(a));
  an_c: cover property (an_seq(b, 2));
  an_m: assume property (@(posedge tck) a |-> b);
  // ST-73: event controls.
  always @(wire1) q = wire1;
  always @(a or b) q = a;
  always @(posedge tck, negedge rst_n) q <= b;
  always @* q = a;
  always @(*) q = b;
  always @(posedge tck iff rst_n) q <= dut.bridge_mode;
  always @(bus.req) q = bus.req;
  initial begin
    @(posedge tck);
    wait (a);
    @(dut.bridge_mode);
    assert (a && b) else $error("x");
    assert property (@(posedge tck) a);
  end
  task automatic an_wait(input logic g);
    @(posedge tck iff g);
  endtask
  assign sel = {an_and(a, b), a};
endmodule
