// llg-test-fixture: IEEE 1800-2009 §§7.4.6, 23.2.2.2, 23.3.2, 23.3.3.
// Fixed output/ref aggregate and terminal shapes retain their typed storage
// identity through selected rows, slices, nested members, and instance arrays.
typedef logic [7:0] lane_t;
typedef lane_t row_t [0:1];
typedef row_t matrix_t [0:1];
typedef lane_t triple_t [0:2];

typedef struct {
    lane_t tag;
    row_t lanes;
} packet_t;

module row_port(input row_t source, output row_t result);
    assign result[0] = source[0] + 8'h01;
    assign result[1] = source[1] + 8'h02;
endmodule

module row_ref(ref row_t value, output lane_t observed);
    assign observed = value[0] ^ value[1];
    initial begin
        #1 value[1] = value[1] + 8'h10;
    end
endmodule

module packet_port(input packet_t source, output packet_t result);
    always_comb begin
        result.tag = source.tag + 8'h01;
        result.lanes[0] = source.lanes[0] ^ 8'h0f;
        result.lanes[1] = source.lanes[1] ^ 8'hf0;
    end
endmodule

module lane_port(input lane_t source, output lane_t result);
    assign result = source + 8'h20;
endmodule

module tb;
    matrix_t source;
    matrix_t direct_result;
    triple_t slice_source;
    triple_t slice_result;
    row_t ref_value;
    lane_t ref_observed;
    packet_t packet_source;
    packet_t packet_result;
    packet_t nested_source;
    packet_t nested_result;
    lane_t scalar_source [0:1];
    lane_t scalar_result [0:1];

    // A constant row actual preserves the remaining fixed-array shape.
    row_port direct(.source(source[0]), .result(direct_result[0]));
    // An unpacked array slice is a fixed row value/target.
    row_port sliced(.source(slice_source[0:1]), .result(slice_result[0:1]));
    // A whole fixed row ref port preserves the selected row identity.
    row_ref referenced(.value(ref_value), .observed(ref_observed));
    packet_port packets(.source(packet_source), .result(packet_result));
    // A nested aggregate member is a legal scalar terminal target.
    lane_port nested(.source(nested_source.lanes[0]), .result(nested_result.lanes[1]));
    // Module instance arrays distribute fixed scalar array elements.
    lane_port distributed[0:1](scalar_source, scalar_result);

    initial begin
        source[0][0] = 8'h10;
        source[0][1] = 8'h20;
        source[1][0] = 8'h30;
        source[1][1] = 8'h40;
        slice_source[0] = 8'h50;
        slice_source[1] = 8'h60;
        slice_source[2] = 8'h70;
        ref_value[0] = 8'h11;
        ref_value[1] = 8'h22;
        packet_source.tag = 8'h01;
        packet_source.lanes[0] = 8'h12;
        packet_source.lanes[1] = 8'h23;
        nested_source.lanes[0] = 8'h34;
        nested_result.lanes[0] = 8'h00;
        scalar_source[0] = 8'h45;
        scalar_source[1] = 8'h56;
        #2;
        $display("direct=%h,%h slice=%h,%h", direct_result[0][0], direct_result[0][1],
                 slice_result[0], slice_result[1]);
        $display("ref=%h,%h observed=%h", ref_value[0], ref_value[1], ref_observed);
        $display("packet=%h,%h,%h", packet_result.tag, packet_result.lanes[0],
                 packet_result.lanes[1]);
        $display("nested=%h distributed=%h,%h", nested_result.lanes[1], scalar_result[0],
                 scalar_result[1]);
        $finish(0);
    end
endmodule
