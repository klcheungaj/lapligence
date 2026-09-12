// IEEE 1800-2009 6.24.3, 7.2-7.3, and 7.5-7.10: packed-element
// dynamic and queue bit-stream casts preserve total width and order.
module tb;
    typedef logic [7:0] word_t;
    typedef logic [3:0] lane_t;
    typedef lane_t lane_dynamic_t[];
    typedef lane_t lane_queue_t[$];
    typedef word_t word_dynamic_t[];
    typedef word_t word_queue_t[$];
    typedef lane_t lane_array_t [0:3];
    word_t packed_source;
    lane_dynamic_t dynamic_dest;
    lane_queue_t queue_dest;
    word_dynamic_t dynamic_source;
    word_queue_t queue_source;
    lane_array_t from_dynamic;
    lane_array_t from_queue;
    initial begin
        packed_source = 8'hcd;
        dynamic_dest = lane_dynamic_t'(packed_source);
        queue_dest = lane_queue_t'(packed_source);
        dynamic_source = '{8'hab, 8'hcd};
        queue_source = '{8'hab, 8'hcd};
        from_dynamic = lane_array_t'(dynamic_source);
        from_queue = lane_array_t'(queue_source);
        if ($size(dynamic_dest) !== 2 || dynamic_dest[0] !== 4'hc ||
            dynamic_dest[1] !== 4'hd || $size(queue_dest) !== 2 ||
            queue_dest[0] !== 4'hc || queue_dest[1] !== 4'hd ||
            from_dynamic[0] !== 4'ha || from_dynamic[1] !== 4'hb ||
            from_dynamic[2] !== 4'hc || from_dynamic[3] !== 4'hd ||
            from_queue[0] !== 4'ha || from_queue[1] !== 4'hb ||
            from_queue[2] !== 4'hc || from_queue[3] !== 4'hd) begin
            $display("FAIL bitstream_containers_probe");
            $finish;
        end
        $display("PASS bitstream_containers_probe");
        $finish;
    end
endmodule
