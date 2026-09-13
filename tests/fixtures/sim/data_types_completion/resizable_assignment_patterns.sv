// IEEE 1800-2009 5.10, 5.11, and 10.9: dynamic arrays and queues infer
// sparse keyed-pattern size from the highest index, while associative arrays
// retain explicit key/value entries.
module tb;
    int dynamic_decl[] = '{default: 2, 1: 3, 4: 5};
    int queue_decl[$] = '{default: 6, 2: 8};
    int associative_decl[int] = '{1: 11, 3: 33};
    int dynamic_value[];
    int queue_value[$];
    int associative_value[int];

    initial begin
        dynamic_value = '{default: 4, 1: 5, 3: 7};
        queue_value = '{default: 9, 2: 12};
        associative_value = '{2: 22, 5: 55};
        if (dynamic_decl.size() != 5
                || dynamic_decl[0] != 2
                || dynamic_decl[1] != 3
                || dynamic_decl[2] != 2
                || dynamic_decl[3] != 2
                || dynamic_decl[4] != 5
                || queue_decl.size() != 3
                || queue_decl[0] != 6
                || queue_decl[1] != 6
                || queue_decl[2] != 8
                || associative_decl[1] != 11
                || associative_decl[3] != 33) begin
            $display("FAIL resizable_assignment_declaration");
            $finish;
        end
        if (dynamic_value.size() != 4
                || dynamic_value[0] != 4
                || dynamic_value[1] != 5
                || dynamic_value[2] != 4
                || dynamic_value[3] != 7
                || queue_value.size() != 3
                || queue_value[0] != 9
                || queue_value[1] != 9
                || queue_value[2] != 12
                || associative_value[2] != 22
                || associative_value[5] != 55) begin
            $display("FAIL resizable_assignment_runtime");
            $finish;
        end
        $display("PASS resizable_assignment_patterns");
        $finish;
    end
endmodule
