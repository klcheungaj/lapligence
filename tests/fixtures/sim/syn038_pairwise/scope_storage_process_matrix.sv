// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/scope_storage_process_matrix.sv
interface process_if(input logic source);
  wire net_value;
  logic variable_value;
  assign net_value = source;
  assign variable_value = source;
endinterface

module tb;
  logic [7:0] source = 8'h00;
  logic trig = 1'b0;
  logic clk = 1'b0;
  logic open = 1'b0;
  logic [7:0] plain_result;
  logic [7:0] comb_result;
  logic [7:0] latch_result;
  logic [7:0] ff_block_result;
  logic [7:0] ff_nba_result;
  logic [7:0] plain_nba_result;
  logic [7:0] event_seen;
  process_if bus(source[0]);

  function automatic logic [7:0] identity(input logic [7:0] value);
    return value;
  endfunction
  function automatic int constant_identity(input int value);
    automatic int per_call = value;
    return per_call;
  endfunction
  localparam int CONST_SOURCE = 5;
  localparam int CONST_RESULT = constant_identity(CONST_SOURCE);

  always @(trig) begin : plain_process
    static logic [7:0] saved;
    automatic logic [7:0] local_value = source;
    saved = local_value;
    plain_result = identity(saved);
    plain_nba_result <= saved;
  end

  always_comb begin : comb_process
    static logic [7:0] saved;
    automatic logic [7:0] local_value = source;
    saved = local_value;
    comb_result = identity(saved);
  end

  always_latch begin : latch_process
    static logic [7:0] saved;
    automatic logic [7:0] local_value = source;
    if (open) begin
      saved = local_value;
      latch_result = identity(saved);
    end
  end

  always_ff @(posedge clk) begin : ff_process
    static logic [7:0] blocking_saved;
    static logic [7:0] nba_saved;
    blocking_saved = source;
    nba_saved <= source;
    ff_block_result <= blocking_saved;
    ff_nba_result <= nba_saved;
  end

  for (genvar g = 0; g < 1; g++) begin : generated
    wire gen_net;
    logic gen_local;
    assign gen_net = source[0];
    initial begin
      #1;
      gen_local = 1'b1;
    end
  end

  initial begin
    @(posedge trig);
    event_seen = source;
  end

  initial begin
    #1;
    source = 8'h5a;
    open = 1'b1;
    trig = 1'b1;
    clk = 1'b1;
    #1;
    if (plain_result !== 8'h5a || plain_nba_result !== 8'h5a ||
        comb_result !== 8'h5a || latch_result !== 8'h5a ||
        ff_block_result !== 8'h5a || event_seen !== 8'h5a ||
        generated[0].gen_local !== 1'b1 || generated[0].gen_net !== 1'b0 ||
        bus.net_value !== 1'b0 || bus.variable_value !== 1'b0 ||
        CONST_RESULT != 5)
      $fatal(1, "phase one");
    source = 8'h5b;
    trig = 1'b0;
    #1;
    trig = 1'b1;
    clk = 1'b0;
    #1;
    clk = 1'b1;
    #1;
    if (plain_result !== 8'h5b || plain_nba_result !== 8'h5b ||
        comb_result !== 8'h5b || latch_result !== 8'h5b ||
        ff_block_result !== 8'h5b || ff_nba_result !== 8'h5a ||
        event_seen !== 8'h5a || generated[0].gen_local !== 1'b1 ||
        generated[0].gen_net !== 1'b1 || bus.net_value !== 1'b1 || bus.variable_value !== 1'b1)
      $fatal(1, "phase two");
    $display("scope=%h,%h,%h,%h,%h,%h,%h event=%h gen=%b,%b if=%b,%b const=%0d",
        plain_result,plain_nba_result,comb_result,latch_result,
        ff_block_result,ff_nba_result,source,event_seen,
        generated[0].gen_local,generated[0].gen_net,bus.net_value,bus.variable_value,
        CONST_RESULT);
    $finish(0);
  end
endmodule
