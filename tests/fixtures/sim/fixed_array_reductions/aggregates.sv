module tb;
    typedef struct packed { bit [3:0] high; logic [3:0] low; } packed_t;
    typedef struct { logic signed [7:0] data; logic [7:0] lanes [0:1]; } record_t;
    packed_t packed_values [0:1];
    record_t records [1:0];
    int data_sum, lane_sum;
    initial begin
        packed_values[0].high = 2; packed_values[0].low = 1;
        packed_values[1].high = 4; packed_values[1].low = 3;
        records[1].data = -3; records[0].data = -4;
        records[1].lanes[0] = 2; records[1].lanes[1] = 3;
        records[0].lanes[0] = 7; records[0].lanes[1] = 11;
        data_sum = records.sum() with (int'(item.data));
        lane_sum = records.sum() with (item.lanes.sum() with (int'(item)));
        $display("packed=%0d field=%0d records=%0d lanes=%0d", packed_values.sum(),
                 packed_values.sum() with (int'(item.high)), data_sum, lane_sum);
    end
endmodule
