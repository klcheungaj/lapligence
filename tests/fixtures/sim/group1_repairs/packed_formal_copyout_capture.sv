module tb;
    typedef struct packed { logic [7:0] tag; logic [1:0][7:0] lanes; } packet_t;
    packet_t source, result;
    function automatic void emit_byte(output logic [7:0] value); value = 8'ha5; endfunction
    function automatic void modify_byte(inout logic [7:0] value); value += 8'd1; endfunction
    function automatic packet_t make(input packet_t copy);
        int index;
        index = 0;
        emit_byte(copy.lanes[index++]);
        if (index != 1 || copy.lanes[0] !== 8'ha5 || copy.lanes[1] !== 8'h20)
            $fatal(1, "output address capture");
        modify_byte(copy.lanes[index++]);
        if (index != 2) $fatal(1, "inout destination evaluated twice");
        return copy;
    endfunction
    initial begin
        source = 24'h7e2010;
        result = make(source);
        if (result !== 24'h7e21a5 || source !== 24'h7e2010)
            $fatal(1, "selected formal copyout");
        $display("packed copyout capture passed");
        $finish(0);
    end
endmodule
