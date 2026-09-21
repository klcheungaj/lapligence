// IEEE 1800-2009 7.4.2, 7.6, and 10.9.1-10.9.2: fixed unpacked-array
// assignment patterns use declaration-order elements, explicit index keys,
// matching type keys, and recursive defaults.  The type-key forms below
// cover local, argument, return, declaration, and nonblocking assignments.
module tb;
    typedef logic [7:0] lane_t;
    typedef logic [64:0] wide65_t;
    typedef logic [128:0] wide129_t;
    typedef lane_t row_t[0:1];
    typedef struct {
        lane_t value;
        lane_t bytes[0:1];
    } record_t;

    lane_t declaration[0:2] = '{lane_t: 8'h22, 0: 8'h11, default: 8'h33};
    row_t rows[0:1] = '{0: '{default: 8'ha1}, row_t: '{default: 8'hb2}};
    record_t records[0:1] = '{record_t: '{
        value: 8'hc1,
        bytes: '{default: 8'hc2}
    }, default: '{value: 8'hd1, bytes: '{default: 8'hd2}}};
    lane_t reversed[-1:1] = '{-1: 8'he1, lane_t: 8'he2, default: 8'he3};
    wide65_t wide65[0:1] = '{wide65_t: 65'h1, default: '0};
    wide129_t wide129[0:1] = '{wide129_t: 129'h2, default: '0};
    lane_t nonblocking[0:2];

    function automatic void check_argument(input lane_t value[0:2]);
        if (value[0] !== 8'h11 || value[1] !== 8'h22 || value[2] !== 8'h22) begin
            $display("FAIL syn_001_argument");
            $finish;
        end
    endfunction

    typedef lane_t array3_t[0:2];

    function automatic array3_t make_return();
        make_return = '{lane_t: 8'h55, default: 8'h66};
    endfunction

    initial begin : test
        lane_t local_value[0:2];
        lane_t returned[0:2];

        local_value = '{0: 8'h71, lane_t: 8'h72, default: 8'h73};
        returned = make_return();
        nonblocking <= '{1: 8'h81, lane_t: 8'h82, default: 8'h83};
        #1;

        check_argument('{0: 8'h11, lane_t: 8'h22, default: 8'h33});
        if (declaration[0] !== 8'h11 || declaration[1] !== 8'h22
                || declaration[2] !== 8'h22
                || local_value[0] !== 8'h71 || local_value[1] !== 8'h72
                || local_value[2] !== 8'h72
                || returned[0] !== 8'h55 || returned[1] !== 8'h55
                || returned[2] !== 8'h55
                || nonblocking[0] !== 8'h82 || nonblocking[1] !== 8'h81
                || nonblocking[2] !== 8'h82
                || rows[0][0] !== 8'ha1 || rows[0][1] !== 8'ha1
                || rows[1][0] !== 8'hb2 || rows[1][1] !== 8'hb2
                || records[0].value !== 8'hc1 || records[0].bytes[0] !== 8'hc2
                || records[0].bytes[1] !== 8'hc2 || records[1].value !== 8'hc1
                || records[1].bytes[0] !== 8'hc2 || records[1].bytes[1] !== 8'hc2
                || reversed[-1] !== 8'he1 || reversed[0] !== 8'he2
                || reversed[1] !== 8'he2
                || wide65[0] !== 65'h1 || wide65[1] !== 65'h1
                || wide129[0] !== 129'h2 || wide129[1] !== 129'h2) begin
            $display("FAIL syn_001_values");
            $finish;
        end
        $display("PASS syn_001_type_keys");
        $finish;
    end
endmodule
