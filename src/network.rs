use std::sync::Arc;

use alumina::ops::activ::*;
use alumina::ops::basic::*;
use alumina::ops::conv::*;
use alumina::ops::loss::*;
use alumina::ops::reshape::*;
use alumina::ops::*;

use alumina::graph::*;

const CHANNELS: usize = 3;

pub fn sr_net(factor: usize, training: Option<(f32, bool)>) -> Graph {
    let mut g = Graph::new();

    let (input, output) = if training.is_some() {
        (
            g.add_node(Node::new_shaped(CHANNELS, 2, "input")),
            g.add_node(Node::new_shaped(CHANNELS, 2, "output")),
        )
    } else {
        (
            g.add_input_node(Node::new_shaped(CHANNELS, 2, "input")),
            g.add_output_node(Node::new_shaped(CHANNELS, 2, "output")),
        )
    };

    g.add_operation(LinearInterp::new(
        &input,
        &output,
        &[factor, factor],
        "linterp",
    ));

    let mut ops: Vec<Box<dyn Operation>> = vec![]; // collect all ops so regularisation can be applied

    let f_conv = g.add_node(Node::new_shaped(32, 2, "f_conv"));
    let f_activ = g.add_node(Node::new_shaped(32, 2, "f_activ"));
    ops.push(Convolution::new(
        &input,
        &f_conv,
        &[5, 5],
        Padding::Same,
        "conv0",
        Convolution::init_msra(1.0),
    ));
    ops.push(Bias::new(
        &f_conv,
        ParamSharing::Spatial,
        "f_bias",
        init_fill(0.0),
    ));
    ops.push(BeLU::new(
        &f_conv,
        &f_activ,
        ParamSharing::Spatial,
        "f_activ",
        BeLU::init_porque_no_los_dos(),
    ));

    let expand = g.add_node(Node::new_shaped(CHANNELS * factor * factor, 2, "expand"));
    ops.push(Bias::new(
        &expand,
        ParamSharing::Spatial,
        "expand_bias",
        init_fill(0.0),
    ));
    ops.push(Expand::new(&expand, &output, &[factor, factor], "expand"));

    for _ in 0..1 {
        // can have multiple independant chains
        let n = 32;
        let l1_conv = g.add_node(Node::new_shaped(n, 2, "l1_conv"));
        let l1_activ = g.add_node(Node::new_shaped(n, 2, "l1_activ"));
        let l2_conv = g.add_node(Node::new_shaped(n, 2, "l2_conv"));
        let l2_activ = g.add_node(Node::new_shaped(n, 2, "l2_activ"));
        let l3_conv = g.add_node(Node::new_shaped(n, 2, "l3_conv"));
        let l3_activ = g.add_node(Node::new_shaped(n, 2, "l3_activ"));

        ops.push(Bias::new(
            &l1_conv,
            ParamSharing::Spatial,
            "l1_bias",
            init_fill(0.0),
        ));
        ops.push(Bias::new(
            &l2_conv,
            ParamSharing::Spatial,
            "l2_bias",
            init_fill(0.0),
        ));
        ops.push(Bias::new(
            &l3_conv,
            ParamSharing::Spatial,
            "l3_bias",
            init_fill(0.0),
        ));

        ops.push(BeLU::new(
            &l1_conv,
            &l1_activ,
            ParamSharing::Spatial,
            "l1_activ",
            BeLU::init_porque_no_los_dos(),
        ));
        ops.push(BeLU::new(
            &l2_conv,
            &l2_activ,
            ParamSharing::Spatial,
            "l2_activ",
            BeLU::init_porque_no_los_dos(),
        ));
        ops.push(BeLU::new(
            &l3_conv,
            &l3_activ,
            ParamSharing::Spatial,
            "l3_activ",
            BeLU::init_porque_no_los_dos(),
        ));

        //-- DenseNet-like Convolution Connections on low-res image
        ops.push(Convolution::new(
            &f_activ,
            &l1_conv,
            &[5, 5],
            Padding::Same,
            "conv1",
            Convolution::init_msra(0.1),
        ));
        ops.push(Convolution::new(
            &f_activ,
            &l2_conv,
            &[5, 5],
            Padding::Same,
            "conv2",
            Convolution::init_msra(0.1),
        ));
        ops.push(Convolution::new(
            &f_activ,
            &l3_conv,
            &[5, 5],
            Padding::Same,
            "conv3",
            Convolution::init_msra(0.1),
        ));
        //ops.push(Convolution::new(&f_activ, &expand, &[3, 3], Padding::Same, "conv4", Convolution::init_msra(0.1))); // seriously slows down training, maybe causes bad conditioning?

        ops.push(Convolution::new(
            &l1_activ,
            &l2_conv,
            &[3, 3],
            Padding::Same,
            "conv5",
            Convolution::init_msra(0.1),
        ));
        ops.push(Convolution::new(
            &l1_activ,
            &l3_conv,
            &[3, 3],
            Padding::Same,
            "conv6",
            Convolution::init_msra(0.1),
        ));
        ops.push(Convolution::new(
            &l1_activ,
            &expand,
            &[3, 3],
            Padding::Same,
            "conv7",
            Convolution::init_msra(0.1),
        ));

        ops.push(Convolution::new(
            &l2_activ,
            &l3_conv,
            &[3, 3],
            Padding::Same,
            "conv8",
            Convolution::init_msra(0.1),
        ));
        ops.push(Convolution::new(
            &l2_activ,
            &expand,
            &[3, 3],
            Padding::Same,
            "conv9",
            Convolution::init_msra(0.1),
        ));

        ops.push(Convolution::new(
            &l3_activ,
            &expand,
            &[3, 3],
            Padding::Same,
            "conv10",
            Convolution::init_msra(0.1),
        ));
    }

    let op_inds = g.add_operations(ops);

    if let Some((regularisation, linear_loss)) = training {
        if regularisation != 0.0 {
            for op_id in &op_inds {
                if op_id.num_params == 0 {
                    continue;
                };
                g.add_secondary_operation(
                    L2Regularisation::new(op_id, regularisation, "L2"),
                    op_id,
                );
            }
        }

        //let _dummy_training_node = g.add_training_input_node(Node::new_flat(1000, "label")); //imagenet only
        let input_hr = g.add_input_node(Node::new_shaped(CHANNELS, 2, "input_hr"));
        let input_hr_lin = g.add_node(Node::new_shaped(CHANNELS, 2, "input_hr_lin"));
        let input_pool = g.add_node(Node::new_shaped(CHANNELS, 2, "input_pool"));
        g.add_operation(SrgbToLinear::new(&input_hr, &input_hr_lin, "srgb2lin"));
        g.add_operation(Pooling::new(
            &input_hr_lin,
            &input_pool,
            &[factor, factor],
            "input_pooling",
        ));
        g.add_operation(LinearToSrgb::new(&input_pool, &input, "lin2srgb"));

        if linear_loss {
            let output_lin = g.add_node(Node::new_shaped(CHANNELS, 2, "output_lin"));
            g.add_operation(SrgbToLinear::new(&output, &output_lin, "srgb2lin_output"));
            g.add_operation(MseLoss::new(&output_lin, &input_hr_lin, 1.0, "loss"));
        } else {
            g.add_operation(MseLoss::new(&output, &input_hr, 1.0, "loss"));
        }

        g.add_operation(ShapeConstraint::new(
            &input_hr,
            &output,
            &[Arc::new(|d| d), Arc::new(|d| d)],
            "output_shape",
        ));
    } else {
        g.add_operation(ShapeConstraint::new(
            &input,
            &output,
            &[Arc::new(move |d| d * factor), Arc::new(move |d| d * factor)],
            "output_shape",
        ));
    }

    g
}
