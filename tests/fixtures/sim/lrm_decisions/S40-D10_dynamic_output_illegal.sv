// Decision S40-D10 (negative case; the expected output is empty because
// compilation fails): a dynamic array is not the actual of an open-array
// formal of output direction.
//
// IEEE 1800-2009 7.7 (SystemVerilog-1800-2009.txt L8579-8580):
//   "A dynamic array or queue shall not be passed as an actual argument if
//   the DPI formal argument has unsized dimensions and an output direction
//   mode."
// Some simulators accept the call and give the formal the size the array has
// at the call; llg follows the clause.
module tb;
    import "DPI-C" function void d10_fill(output int a []);
    int d [];
    initial begin
        d = new[3];
        d10_fill(d);
        $display("d=%p", d);
        $finish;
    end
endmodule
