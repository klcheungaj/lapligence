// llg-lsp-fixture: genvar/shadows.sv
module GenvarShadows;
  genvar index;
  for (index = 0; index < 2; index++) begin : lane
    function automatic int echo(input int index);
      return index;
    endfunction
    typedef struct packed { logic index; } payload_t;
    payload_t payload;
    wire field_value = payload.index;
    GenvarChild child(.index(index));
    if (1) begin : parameter_scope
      localparam integer index = 9;
      wire [3:0] parameter_value = index;
    end
  end
  if (1) begin : local_scope
    genvar index;
    for (index = 0; index < 1; index++) begin : local_loop
      wire local_value = index;
    end
  end
  for (index = 0; index < 3; index++) begin : outside_loop
    wire [1:0] outside_value = index;
  end
endmodule

module GenvarChild(input wire index);
endmodule
