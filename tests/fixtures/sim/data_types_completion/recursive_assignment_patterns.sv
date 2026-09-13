// IEEE 1800-2009 7.2, 7.3, and 10.9.2: recursive assignment patterns use
// nominal member/type keys, nested defaults, and declared array indices. A
// procedural pattern evaluates each source expression once before writes.
module tb;
    typedef logic [7:0] lane_t;
    typedef struct {
        lane_t value;
        logic [7:0] bytes[0:1];
    } inner_t;
    typedef struct {
        inner_t items[0:1];
        lane_t tail;
    } outer_t;

    outer_t declaration = '{
        items: '{
            default: inner_t'('{
                lane_t: 8'ha5,
                bytes: '{default: 8'h3c, 1: 8'h4d}
            })
        },
        tail: 8'h5e
    };
    outer_t expected_declaration = '{
        items: '{
            default: inner_t'('{value: 8'ha5, bytes: '{default: 8'h3c, 1: 8'h4d}})
        },
        tail: 8'h5e
    };
    outer_t assigned;
    outer_t expected_assigned = '{
        items: '{
            default: inner_t'('{value: 8'h1, bytes: '{default: 8'h2}})
        },
        tail: 8'h3
    };
    integer calls;

    function automatic lane_t next_lane();
        calls = calls + 1;
        next_lane = calls[7:0];
    endfunction

    initial begin
        assigned = '{
            items: '{
                default: inner_t'('{
                    value: next_lane(),
                    bytes: '{default: next_lane()}
                })
            },
            tail: next_lane()
        };

        if (declaration != expected_declaration) begin
            $display("FAIL recursive_assignment_declaration");
            $finish;
        end
        // The nested defaults fan out one source node for each distinct
        // pattern expression: value, bytes, and tail.
        if (calls !== 3 || assigned != expected_assigned) begin
            $display("FAIL recursive_assignment_single_evaluation");
            $finish;
        end
        $display("PASS recursive_assignment_patterns");
        $finish;
    end
endmodule
