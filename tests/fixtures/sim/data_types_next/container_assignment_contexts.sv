// IEEE 1800-2009 5.7.1, 6.12.2, 7.5-7.10, 10.8, and 11.6:
// container element targets and method formals provide assignment contexts.
// Associative-array methods return int, and X/Z integral keys are invalid.
module tb;
    typedef logic [7:0] octet_t;
    typedef logic [127:0] wide_t;

    octet_t octet_queue[$];
    wide_t wide_queue[$];
    octet_t octet_dynamic[];
    wide_t wide_dynamic[];
    octet_t associative[logic [7:0]];
    logic [7:0] left;
    logic [7:0] right;
    logic [7:0] x_key;
    logic [7:0] z_key;

    initial begin
        octet_queue = '{'1, 8'h22, 8'h33};
        wide_queue = '{'1};
        octet_dynamic = new[2];
        wide_dynamic = new[2];
        octet_dynamic[0] = '1;
        wide_dynamic[0] = '1;
        if (octet_queue[0] !== 8'hff || wide_queue[0] !== {128{1'b1}} ||
            octet_dynamic[0] !== 8'hff || wide_dynamic[0] !== {128{1'b1}}) begin
            $display("FAIL container_assignment_contexts unbased_fill");
            $finish;
        end

        left = 8'hff;
        right = 8'h01;
        octet_queue.push_back(left + right);
        wide_queue.push_back(left + right);
        octet_dynamic[1] = left + right;
        wide_dynamic[1] = left + right;
        if (octet_queue[3] !== 8'h00 ||
            wide_queue[1] !== {{119{1'b0}}, 9'h100} ||
            octet_dynamic[1] !== 8'h00 ||
            wide_dynamic[1] !== {{119{1'b0}}, 9'h100}) begin
            $display("FAIL container_assignment_contexts carry_width");
            $finish;
        end

        octet_queue.insert(1.5, 4.5);
        wide_queue.push_back(2.5);
        if (octet_queue.size() !== 5 || octet_queue[2] !== 8'h05 ||
            octet_queue[3] !== 8'h33 || wide_queue[2] !== 128'd3) begin
            $display("FAIL container_assignment_contexts real_method_arguments");
            $finish;
        end

        associative[8'h12] = 8'ha5;
        x_key = 8'b0001_x010;
        z_key = 8'b0010_z011;
        associative[x_key] = 8'hff;
        associative[z_key] = 8'hee;
        if ($bits(associative.exists(8'h12)) !== 32 ||
            associative.exists(8'h12) !== 1 ||
            associative.exists(x_key) !== 0 ||
            associative.exists(z_key) !== 0 || associative.num() !== 1 ||
            associative[8'h12] !== 8'ha5) begin
            $display("FAIL container_assignment_contexts associative_keys");
            $finish;
        end

        $display("PASS container_assignment_contexts");
        $finish;
    end
endmodule
