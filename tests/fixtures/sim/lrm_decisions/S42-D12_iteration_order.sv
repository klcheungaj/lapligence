// Decision S42-D12: iteration order. Objects of a scope are returned in the
// order of their names; ports in port order (vpiPortIndex); bits from the
// left index to the right; array elements in row-major order from the left
// bound of each dimension.
//
// IEEE 1800-2009 38.23 (SystemVerilog-1800-2009.txt L63447-63450) and 38.40
// (L65398-65401) define iteration without an order; the text is silent. The
// orders are llg choices; only the port order follows the declaration.
//
// Build S42-D12_iteration_order.c as a VPI application (llg: static archive
// given to --dpi-lib; `task $d12()` declared as a system task).
module child (input logic zz, output logic aa, input logic mm);
    assign aa = zz & mm;
endmodule
module tb;
    logic q, b, a = 1'b1;
    logic [2:5] up = 4'b1100;
    logic [3:0] grid [1:0][0:1];
    child u (.zz(a), .aa(b), .mm(a));
    initial begin
        grid[1][0] = 4'd10;
        grid[1][1] = 4'd11;
        grid[0][0] = 4'd0;
        grid[0][1] = 4'd1;
        $d12;
        $finish;
    end
endmodule
