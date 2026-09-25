// A mixed recursive pattern is a continuous input value, not a fixed storage link.
package types;
    typedef struct { int x; int y; } record_t;
    typedef record_t records_t[2];
endpackage
module child(input types::records_t data, output int result);
    always_comb result = data[0].x + data[0].y + data[1].x + data[1].y;
endmodule
module tb;
    import types::*;
    record_t explicit_value;
    int fill, result;
    child u(.data('{0:explicit_value, int:fill}), .result(result));
    initial begin
        explicit_value = '{11, 22};
        fill = 3;
        #1;
        if (result != 39) $fatal(1, "initial pattern port");
        fill = 5;
        #1;
        if (result != 43) $fatal(1, "pattern fill dependency");
        explicit_value.x = 17;
        #1;
        if (result != 49) $fatal(1, "pattern explicit dependency");
        $display("PASS n03_mixed_pattern_port");
        $finish(0);
    end
endmodule
