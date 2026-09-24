// llg-test-fixture: tests/fixtures/sim/syn038_pairwise/scope_initializer_remainders.sv
interface scope_initializer_if;
  // Focal vector: HC=interface, IN=constant_declaration; gap SYN038-GAP-HC-interface__IN-constant_declaration.
  localparam logic [7:0] interface_constant = 8'h51;

  initial begin : interface_process
    // Focal vector: HC=interface, IN=automatic_local; gap SYN038-GAP-HC-interface__IN-automatic_local.
    automatic logic [7:0] interface_automatic = 8'h62;
    // Focal vector: HC=interface, IN=static_local; gap SYN038-GAP-HC-interface__IN-static_local.
    static logic [7:0] interface_static = 8'h73;
    #3;
    if (interface_constant !== 8'h51 || interface_automatic !== 8'h62 ||
        interface_static !== 8'h73)
      $fatal(1, "interface initializer mismatch");
    $display("interface=%h,%h,%h", interface_constant, interface_automatic,
             interface_static);
  end
endinterface

module tb;
  scope_initializer_if bus();

  function automatic logic [7:0] subroutine_constant;
    // Focal vector: HC=subroutine, IN=constant_declaration; gap SYN038-GAP-HC-subroutine__IN-constant_declaration.
    localparam logic [7:0] subroutine_constant_value = 8'ha1;
    return subroutine_constant_value;
  endfunction

  generate if (1) begin : generated
    // Focal vector: HC=generate, IN=constant_declaration; gap SYN038-GAP-HC-generate__IN-constant_declaration.
    localparam logic [7:0] generate_constant = 8'h11;
    // Focal vector: HC=generate, IN=runtime_declaration; gap SYN038-GAP-HC-generate__IN-runtime_declaration.
    logic [7:0] generate_runtime = 8'h22;

    initial begin : generated_process
      // Focal vector: HC=generate, IN=automatic_local; gap SYN038-GAP-HC-generate__IN-automatic_local.
      automatic logic [7:0] generate_automatic = 8'h33;
      // Focal vector: HC=generate, IN=static_local; gap SYN038-GAP-HC-generate__IN-static_local.
      static logic [7:0] generate_static = 8'h44;
      #2;
      if (generate_constant !== 8'h11 || generate_runtime !== 8'h22 ||
          generate_automatic !== 8'h33 || generate_static !== 8'h44)
        $fatal(1, "generate initializer mismatch");
      $display("generate=%h,%h,%h,%h", generate_constant, generate_runtime,
               generate_automatic, generate_static);
    end
  end endgenerate

  initial begin
    #1;
    if (subroutine_constant() !== 8'ha1)
      $fatal(1, "subroutine localparam mismatch");
    $display("subroutine=%h", subroutine_constant());
    #3;
    $finish(0);
  end
endmodule
