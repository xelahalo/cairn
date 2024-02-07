import numpy as np
import matplotlib.pyplot as plt
from sklearn.linear_model import LinearRegression
import sys
import glob
import json
import os

# read the json file which starts with 'local' 'docker' and 'cairn'
def load_data(filename):
    iters = []
    times = []
    std_devs = []

    with open(filename) as f:
        data = json.load(f)
        for result in data['results']:
            iters.append(int(result['parameters']['iter']))
            times.append(float(result['mean']))
            std_devs.append(float(result['stddev']))

    return np.array(iters), np.array(times), np.array(std_devs)


def count_dirs(dir_path):
    return len([f for f in os.listdir(dir_path) if os.path.isdir(os.path.join(dir_path, f))])


# read the first command argument
result_dir = sys.argv[1]
names = ['local', 
         'docker',
         'fuse_ll_docker', 
         'fuse_docker', 
         'cairn_fuse_no_trace_', 
         'cairn_fuse_trace_', 
         'cairn_II_', 
         'cairn_III_', 
         'fuse_ll_chroot_',
         'fuse_chroot_',
         'cairn_IV_']

colors = [
        (0.00392156862745098, 0.45098039215686275, 0.6980392156862745),
        (0.8705882352941177, 0.5607843137254902, 0.0196078431372549),
        (0.00784313725490196, 0.6196078431372549, 0.45098039215686275),
        (0.8352941176470589, 0.3686274509803922, 0.0),
        (0.8, 0.47058823529411764, 0.7372549019607844), 
        (0.792156862745098, 0.5686274509803921, 0.3803921568627451), 
        (0.984313725490196, 0.6862745098039216, 0.8941176470588236), 
        (0.5803921568627451, 0.5803921568627451, 0.5803921568627451), 
        (0.9254901960784314, 0.8823529411764706, 0.2), 
        (0.33725490196078434, 0.7058823529411765, 0.9137254901960784), 
        (0.00392156862745098, 0.45098039215686275, 0.6980392156862745)
        ]

labelmap = {
    'local': 'Local',
    'docker': 'Docker',
    'fuse_ll_docker': 'FUSE I',
    'fuse_docker': 'FUSE II',
    'cairn_fuse_no_trace_': 'Cairn 0',
    'cairn_fuse_trace_': 'Cairn I',
    'cairn_II_': 'Cairn II',
    'cairn_III_': 'Cairn III',
    'fuse_ll_chroot_': 'FUSE I C',
    'fuse_chroot_': 'FUSE II C',
    'cairn_IV_': 'Cairn IV'
}

for i in range(1, count_dirs(result_dir) + 1):
    for name in names:
        result = glob.glob(f'{result_dir}/{i}/{name}*.json')
        if len(result) == 0:
            continue
        x, y, std = load_data(result[0])
        color = colors[names.index(name)]
        plt.errorbar(x, y, yerr=std,fmt='o', color=color, label=labelmap[name])
        model = LinearRegression().fit(x.reshape(-1, 1), y)
        color = colors[names.index(name)]
        plt.plot(x, model.predict(x.reshape(-1, 1)), color=color, linestyle='dashed', linewidth=2)

    # Add labels and legend
    plt.xlabel('Number of command runs')
    plt.ylabel('Time (seconds)')
    plt.legend()

    # Save the plot as a PNG file
    plt.savefig(f'{result_dir}/{i}/plot.svg')
