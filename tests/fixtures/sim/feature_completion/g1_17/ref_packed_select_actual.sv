// IEEE 1800-2009 13.5.5: a packed bit-select is not a variable storage cell
// and cannot be passed by reference.
module tb;
    logic [7:0] v;

    task automatic set_ref(ref logic [7:0] x);
        x = 8'h2a;
    endtask

    initial begin
        v = 0;
        set_ref(v[0]);
        $display("v=%h", v);
        $finish(0);
    end
endmodule
