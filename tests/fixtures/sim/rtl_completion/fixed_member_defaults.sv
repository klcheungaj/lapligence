module tb;
    localparam int BASE = 5;
    typedef struct {
        int count = BASE + 2;
        logic [7:0] bytes[2] = '{8'h3c, 8'ha5};
        logic valid;
    } value_t;
    value_t global_value;
    value_t elements[2];
    function automatic value_t make();
        value_t local_value;
        return local_value;
    endfunction
    initial begin
        automatic value_t local_value;
        elements[0] = make();
        if (global_value.count !== 7 || global_value.bytes[1] !== 8'ha5) $fatal(1,"global defaults");
        if (local_value.count !== 7 || local_value.bytes[0] !== 8'h3c) $fatal(1,"local defaults");
        if (elements[0].count !== 7 || elements[1].bytes[1] !== 8'ha5) $fatal(1,"array defaults");
        if (global_value.valid !== 1'bx || elements[0].valid !== 1'bx) $fatal(1,"implicit defaults");
        $display("member defaults passed");
        $finish(0);
    end
endmodule
