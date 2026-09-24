// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/hierarchy_process_call_remainders.sv
package hierarchy_remainder_types;
  typedef logic [7:0] byte_t;
  typedef byte_t byte_pair_t [0:1];
endpackage

package constant_scope_cfg;
  localparam int FUNCTION_WIDTH = 5;
  localparam int GENERATE_WIDTH = 7;
  localparam int INTERFACE_WIDTH = 6;
endpackage

interface function_producer_if;
  import hierarchy_remainder_types::*;
  byte_t function_result;

  function automatic byte_t produce_value();
    return 8'ha6;
  endfunction

  initial begin
    #2;
    // Focal vector: HC=interface, CP=function; gap SYN038-GAP-HC-interface__CP-function.
    function_result = produce_value();
  end
endinterface

interface child_value_if(input hierarchy_remainder_types::byte_t source);
  hierarchy_remainder_types::byte_t echoed;
  assign echoed = source;
endinterface

interface child_port_parent_if(input hierarchy_remainder_types::byte_t source);
  // Focal vector: HC=interface, HR=child_port; gap SYN038-GAP-HC-interface__HR-child_port.
  child_value_if nested(source);
endinterface

interface constant_scope_if;
  // Focal vector: CO=constant_elaboration, HC=interface; gap SYN038-GAP-CO-constant_elaboration__HC-interface.
  typedef logic [constant_scope_cfg::INTERFACE_WIDTH-1:0] member_t;
  member_t payload = 6'h2a;
endinterface

module tb;
  import hierarchy_remainder_types::*;

  logic [7:0] child_seed = 8'h5c;
  child_port_parent_if port_bus(child_seed);
  function_producer_if producer_bus();
  constant_scope_if constant_bus();

  function automatic int subroutine_width();
    // Focal vector: CO=constant_elaboration, HC=subroutine; gap SYN038-GAP-CO-constant_elaboration__HC-subroutine.
    typedef logic [constant_scope_cfg::FUNCTION_WIDTH-1:0] local_t;
    return $bits(local_t);
  endfunction

  generate if (1) begin : reduction_scope
    byte_pair_t lanes = '{8'd3, 8'd4};
    byte_t reduction_value;
    initial begin
      #1;
      // Focal vector: HC=generate, CP=fixed_array_reduction; gap SYN038-GAP-HC-generate__CP-fixed_array_reduction.
      reduction_value = lanes.sum();
      if (lanes[0] !== 8'd3 || lanes[1] !== 8'd4 || reduction_value !== 8'd7)
        $fatal(1, "generated reduction receiver or result mismatch");
    end
  end endgenerate

  generate if (1) begin : latch_scope
    logic enable = 1'b0;
    byte_t source = 8'h00;
    byte_t latch_value;

    // Focal vector: HC=generate, PC=always_latch; gap SYN038-GAP-HC-generate__PC-always_latch.
    always_latch begin
      if (enable)
        latch_value = source;
    end
  end endgenerate

  for (genvar g = 0; g < 1; g++) begin : constant_generate_scope
    // Focal vector: CO=constant_elaboration, HC=generate; gap SYN038-GAP-CO-constant_elaboration__HC-generate.
    typedef logic [constant_scope_cfg::GENERATE_WIDTH-1:0] generated_t;
    generated_t payload = 7'h5a;
  end

  initial begin
    #1;
    latch_scope.source = 8'h91;
    latch_scope.enable = 1'b1;
    #1;
    if (latch_scope.latch_value !== 8'h91 || reduction_scope.reduction_value !== 8'd7)
      $fatal(1, "generated process or reduction mismatch");
    #1;
    if (producer_bus.function_result !== 8'ha6 || port_bus.nested.echoed !== 8'h5c ||
        subroutine_width() != 5 || $bits(constant_generate_scope[0].payload) != 7 ||
        $bits(constant_bus.payload) != 6 || constant_bus.payload !== 6'h2a)
      $fatal(1, "hierarchy, function, or constant elaboration mismatch");
    $display("reduction=%h function=%h latch=%h child_port=%h const=%0d,%0d,%h",
             reduction_scope.reduction_value, producer_bus.function_result,
             latch_scope.latch_value, port_bus.nested.echoed, subroutine_width(),
             $bits(constant_generate_scope[0].payload), constant_bus.payload);
    $finish(0);
  end
endmodule
