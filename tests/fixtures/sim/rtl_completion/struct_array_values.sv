module tb;
    typedef struct { int tag; logic [7:0] data; } packet_t;
    packet_t values [2:1];
    function automatic int sum(input packet_t packets[2:1]);
        return packets[2].tag + packets[1].tag;
    endfunction
    initial begin
        values[2].data=0;
        if (values[2] !== packet_t'{tag:0, data:0}) $fatal(1, "mixed-state element default");
        values[2].tag=7; values[2].data=8'h5a;
        values[1].tag=9; values[1].data=8'ha5;
        $display("sum=%0d data=%h,%h", sum(values), values[2].data, values[1].data);
        $finish(0);
    end
endmodule
