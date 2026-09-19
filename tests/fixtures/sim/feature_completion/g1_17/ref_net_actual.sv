// IEEE 1800-2009 13.5.5: a `ref` actual must be an integral variable. A net
// (wire/tri) is driven storage, not a variable, so it is not a legal ref
// actual.
module tb;
    wire [7:0] w;
    logic [7:0] src;
    assign w = src;

    task automatic set_ref(ref logic [7:0] x);
        x = 8'h2a;
    endtask

    initial begin
        src = 0;
        set_ref(w);
        $display("w=%h", w);
        $finish(0);
    end
endmodule
