// Decision S3-D3: a UDP input that changes between z and x has not changed:
// the table is not evaluated, so even a row that would change the state for
// the same inputs does not fire. A change between 0 or 1 and z is the same
// transition as to or from x.
//
// IEEE 1800-2009 29.3.5 (SystemVerilog-1800-2009.txt L48808-48809): "The z
// values passed to UDP inputs shall be treated the same as x values." The
// rows `x 1 : 0 : 1` and `x 1 : 1 : 0` toggle the state on every evaluation
// with c = x, so an evaluation on x -> z or z -> x would show.
primitive toggle_on_x (q, c, e);
    output q;
    reg q;
    input c, e;
    initial q = 0;
    table
    // c e : q : q+
        x 1 : 0 : 1 ;
        x 1 : 1 : 0 ;
        0 ? : ? : - ;
        1 ? : ? : - ;
        x 0 : ? : - ;
    endtable
endprimitive

module tb;
    reg c, e;
    wire q;
    toggle_on_x u (q, c, e);
    initial begin
        c = 1'b0;
        #1 e = 1'b1;
        #1 $display("start q=%b", q);
        c = 1'bx;
        #1 $display("0_to_x q=%b", q);
        c = 1'bz;
        #1 $display("x_to_z q=%b", q);
        c = 1'bx;
        #1 $display("z_to_x q=%b", q);
        c = 1'b0;
        #1 $display("x_to_0 q=%b", q);
        c = 1'bz;
        #1 $display("0_to_z q=%b", q);
        $finish;
    end
endmodule
