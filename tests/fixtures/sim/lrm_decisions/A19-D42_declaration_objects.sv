// Decision A19-D42: generate scope arrays, genvars, io declarations and
// specparams as objects (IEEE 1800-2009 37.13, 37.26, 37.80).
// A generate loop is a vpiGenScopeArray: vpiSize counts its generated
// scopes, vpiGenVar is its genvar, vpi_handle_by_index gives the scope of
// a genvar value (NULL for a value the loop did not generate) and each
// scope has it as vpiGenScopeArray. A genvar has a name and full name.
// A module, task, function and modport iterate their io declarations in
// declaration order: vpiDirection (vpiRef for a ref argument), the value
// shape, vpiExpr (the port's declaration, the interface member of a
// modport port), vpiTaskFunc and vpiModport (NULL without an error when
// the declaration is not in one). An io declaration has no vpiFullName
// (vpiError). A specparam is a vpiSpecParam with its value.
interface bus;
  logic [3:0] data;
  logic ready;
  modport src (output data, input ready);
endinterface
module leaf (input logic [7:4] a, output logic y);
  specify
    specparam tpd = 3;
  endspecify
  assign y = a[4];
endmodule
module tb;
  logic [7:4] a = 4'b0101;
  logic y;
  bus b ();
  leaf u (.a(a), .y(y));
  genvar g;
  for (g = 2; g < 5; g = g + 1) begin : gb
    logic [g:0] w;
  end
  function automatic int scale(input int x, ref logic [1:0] r);
    return x * 2;
  endfunction
  initial begin
    #1 $d42;
    $display("y=%b", y);
    $finish;
  end
endmodule
