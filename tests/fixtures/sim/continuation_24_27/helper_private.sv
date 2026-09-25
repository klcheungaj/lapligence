// llg-test-fixture: private concat/pattern writes and transitive event dependencies.
`ifndef CONTINUATION_HELPER_W
`define CONTINUATION_HELPER_W 65
`endif
module tb;
    localparam W = `CONTINUATION_HELPER_W;
    typedef logic [W-1:0] lane_t;
    typedef lane_t row_t [0:1];
    row_t source;
    lane_t bias;
    logic choose;
    integer changes, rises;
    function automatic lane_t computed(input row_t values, const ref lane_t adjustment);
        lane_t high_value, low_value;
        row_t private_values;
        {high_value,low_value} = {values[0],values[1]};
        row_t'{private_values[0],private_values[1]} = values;
        private_values[0] ^= high_value;
        low_value += adjustment;
        unique case (choose)
            1'b0: return low_value;
            1'b1: return high_value ^ adjustment;
            default: return private_values[1];
        endcase
    endfunction
    function automatic lane_t outer(input row_t values, const ref lane_t adjustment);
        return computed(values, adjustment);
    endfunction
    always @(outer(source,bias)) changes++;
    always @(posedge outer(source,bias)) rises++;
    initial begin
        source='{1,4}; bias=1; choose=0; changes=0; rises=0;
        #1; changes=0; rises=0;
        source[1]=5; #1;
        if (changes != 1 || outer(source,bias) !== lane_t'(6)) $fatal(1,"private helper value");
        source[0]=3; #1;
        if (changes != 1) $fatal(1,"dependency change with unchanged helper result");
        bias=2; #1;
        if (changes != 2 || rises != 1) $fatal(1,"const ref or event edge lost");
        choose=1; #1;
        if (changes != 3 || outer(source,bias) !== lane_t'(1)) $fatal(1,"transitive global read lost");
        source[1]=9; #1;
        if (changes != 3) $fatal(1,"unselected result published");
        choose=0; #1;
        if (changes != 4 || rises != 1 || outer(source,bias) !== lane_t'(11))
            $fatal(1,"private state escaped or input copy stale");
        $display("HELPERS_PASS W=%0d",W); $finish(0);
    end
endmodule
