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
names = ['local', 'docker', 'fuse_ll_docker', 'fuse_docker', 'cairn_I', 'cairn_II', 'cairn_III', 'cairn_IV']
colors = ['green', 'blue', 'darkblue', 'midnightblue', 'rosybrown', 'indianred','firebrick', 'darkred']
labelmap = {
    'local': 'Running command locally',
    'docker': 'Running command in Docker',
    'fuse_ll_docker': 'Running command in Docker on top of FUSE (lowlevel)',
    'fuse_docker': 'Running command in Docker on top of FUSE',
    'cairn_I': 'Running command on the Cairn FUSE layer',
    'cairn_II': 'Running command on the Cairn FUSE layer (from host)',
    'cairn_III': 'Running command on the Cairn FUSE layer (from host with chroot)',
    'cairn_IV': 'Running the command with Cairn client',
}

for i in range(1, count_dirs(result_dir) + 1):
    for name in names:
        print(name)
        result = glob.glob(f'{result_dir}/{i}/{name}*.json')
        x, y, std = load_data(result[0])
        color = colors[names.index(name)]
        plt.errorbar(x, y, yerr=std, fmt='o', color=color, label=labelmap[name])
        model = LinearRegression().fit(x.reshape(-1, 1), y)
        plt.plot(x, model.predict(x.reshape(-1, 1)), color=color, linestyle='dashed', linewidth=2)

    # Add labels and legend
    plt.xlabel('Number of iterations')
    plt.ylabel('Time (seconds)')
    plt.legend()

    # Save the plot as a PNG file
    plt.savefig(f'{result_dir}/{i}/plot.png')
