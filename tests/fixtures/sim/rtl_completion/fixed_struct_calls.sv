module tb;
    typedef struct {
        bit [3:0] tag;
        int payload[1:-1];
    } packet_t;
    packet_t source, result;
    function automatic packet_t transform(input packet_t input_value);
        packet_t local_value = input_value;
        local_value.tag = 4'hf;
        local_value.payload[0] += 3;
        return local_value;
    endfunction
    task automatic update(ref packet_t value, const ref packet_t observer);
        value.payload[1] = 9;
        if (observer.payload[1] != source.payload[1]) $fatal(1, "struct reference visibility");
    endtask
    initial begin
        source = '{tag:4'h2, payload:'{1,2,3}};
        result = transform(source);
        update(source, source);
        $display("source=%h,%0d,%0d,%0d", source.tag, source.payload[1], source.payload[0], source.payload[-1]);
        $display("result=%h,%0d,%0d,%0d", result.tag, result.payload[1], result.payload[0], result.payload[-1]);
        $finish(0);
    end
endmodule
