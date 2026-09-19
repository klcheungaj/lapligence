// IEEE 1800-2009 13.5.5: a ref formal and its actual must have equivalent
// types; a narrowing actual is rejected rather than aliased.
module tb;
    logic [3:0] v;

    task automatic set_ref(ref logic [7:0] x);
        x = 8'h2a;
    endtask

    initial begin
        v = 0;
        set_ref(v);
        $display("v=%h", v);
        $finish(0);
    end
endmodule
